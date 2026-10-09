//! Selected-source metadata indexing. No audio or thumbnail files are fetched.

use super::{TdJson, playable_music_message};
use crate::adapters::sqlite::library::{LibraryStore, MediaFilter, TrackInput};
use music_core::domain::{LibraryCommand, LibrarySnapshot, SourceSnapshot};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const PAGE_SIZE: i64 = 50;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Clone, Debug)]
enum Job {
    Scan {
        chat: i64,
        filter: MediaFilter,
        cursor: i64,
    },
    Refresh {
        chat: i64,
        message: i64,
    },
}

impl Job {
    fn chat(&self) -> i64 {
        match self {
            Self::Scan { chat, .. } | Self::Refresh { chat, .. } => *chat,
        }
    }
}

pub(crate) fn parse_track(message: &Value) -> Option<TrackInput> {
    if !playable_music_message(message) {
        return None;
    }
    let message_id = message["id"].as_i64()?;
    let date = message["date"].as_i64().unwrap_or(0);
    let content = &message["content"];
    let (media_type, info) = if content["@type"] == "messageAudio" {
        ("audio", &content["audio"])
    } else {
        ("document", &content["document"])
    };
    let filename = info["file_name"].as_str().unwrap_or("").trim().to_string();
    let raw_title = info["title"].as_str().unwrap_or("").trim();
    let title = if !raw_title.is_empty() {
        raw_title.to_string()
    } else if !filename.is_empty() {
        Path::new(&filename)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or(&filename)
            .to_string()
    } else {
        "Untitled track".into()
    };
    let artist = info["performer"]
        .as_str()
        .filter(|artist| !artist.trim().is_empty())
        .unwrap_or("Unknown artist")
        .trim()
        .to_string();
    let duration_seconds = info["duration"].as_i64().filter(|duration| *duration > 0);
    let cover_data = info["album_cover_minithumbnail"]["data"]
        .as_str()
        .filter(|data| !data.is_empty() && data.len() < 32_768)
        .map(str::to_string);
    Some(TrackInput {
        message_id,
        date,
        media_type,
        title,
        artist,
        filename,
        duration_seconds,
        cover_data,
    })
}

pub struct LibraryEngine {
    store: Option<LibraryStore>,
    updates: watch::Sender<LibrarySnapshot>,
    account: Option<i64>,
    selected: BTreeSet<i64>,
    queued: VecDeque<Job>,
    active: BTreeMap<i64, (Job, Instant)>,
    loaded: usize,
    active_initial: Option<char>,
    error: Option<String>,
    pause_until: Option<Instant>,
}

impl LibraryEngine {
    pub fn new(path: &str, updates: watch::Sender<LibrarySnapshot>) -> Self {
        let store = LibraryStore::open(Path::new(path));
        let error = store
            .as_ref()
            .err()
            .map(|_| "Can't open the music library. Check device storage and restart.".into());
        Self {
            store: store.ok(),
            updates,
            account: None,
            selected: BTreeSet::new(),
            queued: VecDeque::new(),
            active: BTreeMap::new(),
            loaded: 50,
            active_initial: None,
            error,
            pause_until: None,
        }
    }

    fn publish(&self) {
        let mut snapshot = LibrarySnapshot {
            account_id: self.account,
            active_initial: self.active_initial,
            selected_sources: self.selected.len(),
            error: self.error.clone(),
            ..Default::default()
        };
        if let (Some(store), Some(account)) = (&self.store, self.account) {
            match store.page(account, self.loaded, self.active_initial) {
                Ok((tracks, count)) => {
                    snapshot.total_count = count;
                    snapshot.has_more = count > tracks.len();
                    snapshot.tracks = tracks;
                }
                Err(_) => {
                    snapshot.error = Some(
                        "Can't read the music library. Check device storage and restart.".into(),
                    )
                }
            }
        }
        snapshot.indexing_sources = self
            .queued
            .iter()
            .chain(self.active.values().map(|(job, _)| job))
            .filter_map(|job| match job {
                Job::Scan { chat, .. } => Some(*chat),
                _ => None,
            })
            .collect::<BTreeSet<_>>()
            .len();
        self.updates.send_replace(snapshot);
    }

    pub fn reset(&mut self) {
        self.account = None;
        self.selected.clear();
        self.queued.clear();
        self.active.clear();
        self.pause_until = None;
        self.loaded = 50;
        self.active_initial = None;
        if self.store.is_some() {
            self.error = None;
        }
        self.publish();
    }

    pub fn sync_sources(&mut self, sources: &SourceSnapshot) {
        let account = sources
            .account_id
            .filter(|_| sources.setup_complete && !sources.signing_out);
        if account != self.account {
            self.reset();
            self.account = account;
        }
        let desired: BTreeSet<_> = if account.is_some() {
            sources
                .chats
                .iter()
                .filter(|chat| chat.selected)
                .map(|chat| chat.chat_id)
                .collect()
        } else {
            BTreeSet::new()
        };
        if desired == self.selected && account == self.updates.borrow().account_id {
            return;
        }
        self.queued.retain(|job| desired.contains(&job.chat()));
        self.active
            .retain(|_, (job, _)| desired.contains(&job.chat()));
        if let (Some(store), Some(account)) = (&self.store, account) {
            for chat in desired.difference(&self.selected) {
                match store.start_source(account, *chat, false) {
                    Ok(jobs) => {
                        for (filter, cursor) in jobs {
                            self.queued.push_back(Job::Scan {
                                chat: *chat,
                                filter,
                                cursor,
                            });
                        }
                    }
                    Err(_) => {
                        self.error =
                            Some("Can't start indexing this source. Check device storage.".into())
                    }
                }
            }
        }
        self.selected = desired;
        self.publish();
    }

    pub fn command(&mut self, command: LibraryCommand) {
        match command {
            LibraryCommand::LoadMore => {
                self.loaded = self.loaded.saturating_add(50);
                self.publish();
            }
            LibraryCommand::SelectInitial(letter) => {
                if !letter.is_ascii_alphabetic() {
                    return;
                }
                let letter = letter.to_ascii_uppercase();
                self.active_initial = if self.active_initial == Some(letter) {
                    None
                } else {
                    Some(letter)
                };
                self.loaded = 50;
                self.publish();
            }
            LibraryCommand::Reindex => {
                let scanning = self
                    .queued
                    .iter()
                    .any(|job| matches!(job, Job::Scan { .. }))
                    || self
                        .active
                        .values()
                        .any(|(job, _)| matches!(job, Job::Scan { .. }));
                if scanning || self.error.is_some() || self.selected.is_empty() {
                    return;
                }
                if let (Some(store), Some(account)) = (&self.store, self.account) {
                    for chat in &self.selected {
                        match store.start_source(account, *chat, true) {
                            Ok(jobs) => {
                                for (filter, cursor) in jobs {
                                    self.queued.push_back(Job::Scan {
                                        chat: *chat,
                                        filter,
                                        cursor,
                                    });
                                }
                            }
                            Err(_) => {
                                self.error = Some(
                                    "Can't start reindexing. Check device storage and retry."
                                        .into(),
                                )
                            }
                        }
                    }
                }
                self.publish();
            }
            LibraryCommand::Retry => {
                self.error = None;
                self.queued.clear();
                self.active.clear();
                if let (Some(store), Some(account)) = (&self.store, self.account) {
                    for chat in &self.selected {
                        match store.start_source(account, *chat, false) {
                            Ok(jobs) => {
                                for (filter, cursor) in jobs {
                                    self.queued.push_back(Job::Scan {
                                        chat: *chat,
                                        filter,
                                        cursor,
                                    });
                                }
                            }
                            Err(_) => {
                                self.error =
                                    Some("Can't restart indexing. Check device storage.".into())
                            }
                        }
                    }
                }
                self.publish();
            }
        }
    }

    pub fn observe_update(&mut self, value: &Value) {
        let Some(account) = self.account else {
            return;
        };
        let Some(store) = &self.store else {
            return;
        };
        let kind = value["@type"].as_str().unwrap_or("");
        let changed = match kind {
            "updateNewMessage" => {
                let message = &value["message"];
                let chat = message["chat_id"].as_i64();
                if let (Some(chat), Some(track)) = (chat, parse_track(message)) {
                    if self.selected.contains(&chat) {
                        store.upsert_live(account, chat, &track).is_ok()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "updateMessageContent" => {
                if let (Some(chat), Some(message)) =
                    (value["chat_id"].as_i64(), value["message_id"].as_i64())
                    && self.selected.contains(&chat)
                {
                    self.queued.push_front(Job::Refresh { chat, message });
                }
                false
            }
            "updateDeleteMessages" if value["from_cache"] != true => {
                let chat = value["chat_id"].as_i64();
                if let Some(chat) = chat.filter(|chat| self.selected.contains(chat)) {
                    let mut changed = false;
                    if let Some(ids) = value["message_ids"].as_array() {
                        for id in ids.iter().filter_map(Value::as_i64) {
                            changed |= store.remove_message(account, chat, id).is_ok();
                        }
                    }
                    changed
                } else {
                    false
                }
            }
            _ => false,
        };
        if changed {
            self.publish();
        }
    }

    pub fn handle_response(&mut self, value: &Value) -> bool {
        let Some(extra) = value["@extra"].as_i64() else {
            return false;
        };
        let Some((job, _)) = self.active.remove(&extra) else {
            return false;
        };
        let Some(account) = self.account else {
            return true;
        };
        if !self.selected.contains(&job.chat()) {
            return true;
        }
        if value["@type"] == "error" {
            if let Job::Refresh { chat, message } = &job
                && value["code"] == 404
            {
                if let Some(store) = &self.store {
                    let _ = store.remove_message(account, *chat, *message);
                }
                self.publish();
                return true;
            }
            let reason = value["message"].as_str().unwrap_or("");
            if reason.to_ascii_uppercase().contains("FLOOD_WAIT") || value["code"] == 429 {
                let seconds = reason
                    .split(|c: char| !c.is_ascii_digit())
                    .find_map(|part| part.parse::<u64>().ok())
                    .unwrap_or(5)
                    .clamp(1, 60);
                self.pause_until = Some(Instant::now() + Duration::from_secs(seconds));
                self.queued.push_front(job);
            } else {
                self.error = Some(
                    "Couldn't index a Telegram source. Check the connection and retry.".into(),
                );
            }
            self.publish();
            return true;
        }
        let Some(store) = &mut self.store else {
            return true;
        };
        let result = match job {
            Job::Scan {
                chat,
                filter,
                cursor,
            } => {
                let next = value["next_from_message_id"].as_i64();
                if value["messages"].as_array().is_none()
                    || !next.is_some_and(|next| next == 0 || (next > 0 && next != cursor))
                {
                    Err("Telegram returned an invalid music page.".to_string())
                } else {
                    let next = next.unwrap_or(0);
                    let tracks: Vec<_> = value["messages"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(parse_track)
                        .collect();
                    store
                        .save_page(account, chat, filter, next, &tracks)
                        .map(|_| {
                            if next != 0 {
                                self.queued.push_back(Job::Scan {
                                    chat,
                                    filter,
                                    cursor: next,
                                });
                            }
                        })
                }
            }
            Job::Refresh { chat, message } => {
                if let Some(track) = parse_track(value) {
                    store.upsert_live(account, chat, &track)
                } else {
                    store.remove_message(account, chat, message)
                }
            }
        };
        if result.is_err() {
            self.error =
                Some("Couldn't save a music index page. Check device storage and retry.".into());
        }
        self.publish();
        true
    }

    pub fn drive(&mut self, td: &TdJson) {
        if self.error.is_some() {
            return;
        }
        if self.pause_until.is_some_and(|until| Instant::now() < until) {
            return;
        }
        self.pause_until = None;
        let timed_out: Vec<_> = self
            .active
            .iter()
            .filter(|(_, (_, since))| since.elapsed() > REQUEST_TIMEOUT)
            .map(|(id, _)| *id)
            .collect();
        if !timed_out.is_empty() {
            for id in timed_out {
                self.active.remove(&id);
            }
            self.error = Some(
                "Telegram stopped responding while indexing. Check the connection and retry."
                    .into(),
            );
            self.publish();
            return;
        }
        while self.active.len() < 2 {
            let Some(job) = self.queued.pop_front() else {
                break;
            };
            if !self.selected.contains(&job.chat()) {
                continue;
            }
            let request = match job {
                Job::Scan {
                    chat,
                    filter,
                    cursor,
                } => json!({
                    "@type":"searchChatMessages", "chat_id":chat, "topic_id":null,
                    "query":"", "sender_id":null, "from_message_id":cursor,
                    "offset":0, "limit":PAGE_SIZE,
                    "filter":{"@type":if filter == MediaFilter::Audio {
                        "searchMessagesFilterAudio"
                    } else { "searchMessagesFilterDocument" }}
                }),
                Job::Refresh { chat, message } => json!({
                    "@type":"getMessage", "chat_id":chat, "message_id":message
                }),
            };
            let extra = td.next_request_id();
            let mut request = request;
            request["@extra"] = json!(extra);
            if td.send(&request).is_ok() {
                self.active.insert(extra, (job, Instant::now()));
            } else {
                self.queued.push_front(job);
                self.error =
                    Some("Couldn't request Telegram music. Check the connection and retry.".into());
                self.publish();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite::SourceStore;
    use music_core::domain::{MusicCheck, SourceChat, SourceKind};

    #[test]
    fn parses_audio_document_and_fallbacks() {
        let audio = json!({"id":7,"date":10,"content":{"@type":"messageAudio","audio":{
            "file_name":"sample.mp3","title":" Persian  ","performer":"Artist","duration":90,
            "album_cover_minithumbnail":{"data":"abc"}
        }}});
        let track = parse_track(&audio).unwrap();
        assert_eq!(track.title, "Persian");
        assert_eq!(track.artist, "Artist");
        assert_eq!(track.cover_data.as_deref(), Some("abc"));
        let document = json!({"id":8,"date":11,"content":{"@type":"messageDocument","document":{
            "mime_type":"audio/flac","file_name":"آهنگ.flac"
        }}});
        let track = parse_track(&document).unwrap();
        assert_eq!(track.title, "آهنگ");
        assert_eq!(track.artist, "Unknown artist");
        assert_eq!(track.media_type, "document");
        assert!(parse_track(&json!({"id":9,"content":{"@type":"messageVoiceNote"}})).is_none());
    }

    #[test]
    fn short_page_uses_telegram_cursor_and_persists_track() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-library-cursor-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let source_store = SourceStore::open(&path).unwrap();
        source_store
            .set_selected(
                7,
                &SourceChat {
                    chat_id: 42,
                    title: "Music bot".into(),
                    subtitle: String::new(),
                    kind: SourceKind::MusicBot,
                    selected: true,
                    music: MusicCheck::Found,
                },
                true,
            )
            .unwrap();
        let (updates, _) = watch::channel(LibrarySnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates);
        let mut sources = SourceSnapshot {
            account_id: Some(7),
            setup_complete: true,
            ..Default::default()
        };
        sources.chats.push(SourceChat {
            chat_id: 42,
            title: "Music bot".into(),
            subtitle: String::new(),
            kind: SourceKind::MusicBot,
            selected: true,
            music: MusicCheck::Found,
        });
        engine.sync_sources(&sources);
        engine.drive(&TdJson);
        let audio_extra = *engine
            .active
            .iter()
            .find(|(_, (job, _))| {
                matches!(
                    job,
                    Job::Scan {
                        filter: MediaFilter::Audio,
                        ..
                    }
                )
            })
            .unwrap()
            .0;
        assert!(engine.handle_response(&json!({
            "@extra":audio_extra,"@type":"foundChatMessages","next_from_message_id":77,
            "messages":[{"id":90,"chat_id":42,"date":100,"content":{
                "@type":"messageAudio","audio":{"title":"First","file_name":"first.mp3"}
            }}]
        })));
        assert!(engine.queued.iter().any(|job| matches!(
            job,
            Job::Scan {
                chat: 42,
                filter: MediaFilter::Audio,
                cursor: 77
            }
        )));
        assert_eq!(
            engine.store.as_ref().unwrap().page(7, 50, None).unwrap().1,
            1
        );
        drop(engine);
        drop(source_store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn completed_catalog_waits_for_manual_reindex() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-library-manual-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let source_store = SourceStore::open(&path).unwrap();
        let chat = SourceChat {
            chat_id: 42,
            title: "Music bot".into(),
            subtitle: String::new(),
            kind: SourceKind::MusicBot,
            selected: true,
            music: MusicCheck::Found,
        };
        source_store.set_selected(7, &chat, true).unwrap();
        let mut store = LibraryStore::open(&path).unwrap();
        store.start_source(7, 42, false).unwrap();
        store.save_page(7, 42, MediaFilter::Audio, 0, &[]).unwrap();
        store
            .save_page(7, 42, MediaFilter::Document, 0, &[])
            .unwrap();
        drop(store);
        let (updates, receiver) = watch::channel(LibrarySnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates);
        let sources = SourceSnapshot {
            account_id: Some(7),
            setup_complete: true,
            chats: vec![chat],
            ..Default::default()
        };
        engine.sync_sources(&sources);
        assert!(engine.queued.is_empty());
        assert_eq!(receiver.borrow().selected_sources, 1);
        engine.command(LibraryCommand::Reindex);
        assert_eq!(engine.queued.len(), 2);
        assert_eq!(receiver.borrow().indexing_sources, 1);
        engine.command(LibraryCommand::Reindex);
        assert_eq!(engine.queued.len(), 2);
        drop(engine);
        drop(source_store);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn letter_selection_resets_paging_and_tracks_live_index_updates() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-library-initial-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let source_store = SourceStore::open(&path).unwrap();
        let chat = SourceChat {
            chat_id: 42,
            title: "Music bot".into(),
            subtitle: String::new(),
            kind: SourceKind::MusicBot,
            selected: true,
            music: MusicCheck::Found,
        };
        source_store.set_selected(7, &chat, true).unwrap();
        let (updates, receiver) = watch::channel(LibrarySnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates);
        engine.sync_sources(&SourceSnapshot {
            account_id: Some(7),
            setup_complete: true,
            chats: vec![chat],
            ..Default::default()
        });
        engine.command(LibraryCommand::LoadMore);
        assert_eq!(engine.loaded, 100);
        engine.command(LibraryCommand::SelectInitial('a'));
        assert_eq!(engine.loaded, 50);
        assert_eq!(receiver.borrow().active_initial, Some('A'));
        assert_eq!(receiver.borrow().indexing_sources, 1);
        for (id, title) in [(1, "Amber"), (2, "Blue"), (3, "apricot")] {
            engine.observe_update(&json!({
                "@type":"updateNewMessage", "message":{
                    "id":id, "chat_id":42, "date":id,
                    "content":{"@type":"messageAudio", "audio":{
                        "title":title, "file_name":format!("{id}.mp3")
                    }}
                }
            }));
        }
        assert_eq!(receiver.borrow().active_initial, Some('A'));
        assert_eq!(receiver.borrow().total_count, 2);
        assert_eq!(receiver.borrow().tracks[0].title, "Amber");
        engine.command(LibraryCommand::SelectInitial('Z'));
        assert_eq!(receiver.borrow().active_initial, Some('Z'));
        assert_eq!(receiver.borrow().total_count, 0);
        engine.command(LibraryCommand::SelectInitial('Z'));
        assert_eq!(receiver.borrow().active_initial, None);
        assert_eq!(receiver.borrow().total_count, 3);
        engine.command(LibraryCommand::SelectInitial('1'));
        assert_eq!(receiver.borrow().active_initial, None);
        drop(engine);
        drop(source_store);
        std::fs::remove_file(path).unwrap();
    }
}

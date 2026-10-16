//! Selected-source metadata indexing and bounded cover-thumbnail downloads.

use super::{TdJson, playable_music_message};
use crate::adapters::sqlite::library::{LibraryStore, MediaFilter, TrackInput};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use music_core::domain::{
    LibraryCommand, LibrarySnapshot, LibrarySort, SearchCommand, SearchSnapshot, SourceSnapshot,
    TrackKey, TrackSummary,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const PAGE_SIZE: i64 = 50;
const HOME_RECENT_LIMIT: usize = 12;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(35);
const BACKFILL_PAGE_SIZE: usize = 200;

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
    Backfill {
        chat: i64,
        message: i64,
    },
    Cover {
        chat: i64,
        message: i64,
        file_id: i64,
    },
}

impl Job {
    fn chat(&self) -> i64 {
        match self {
            Self::Scan { chat, .. }
            | Self::Refresh { chat, .. }
            | Self::Backfill { chat, .. }
            | Self::Cover { chat, .. } => *chat,
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
    let mini = if media_type == "audio" {
        &info["album_cover_minithumbnail"]
    } else {
        &info["minithumbnail"]
    };
    let cover_data = mini["data"]
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

fn cover_file_id(message: &Value) -> Option<i64> {
    let content = &message["content"];
    if content["@type"] == "messageAudio" {
        let audio = &content["audio"];
        audio["album_cover_thumbnail"]["file"]["id"]
            .as_i64()
            .or_else(|| {
                audio["external_album_covers"]
                    .as_array()?
                    .iter()
                    .find_map(|cover| cover["file"]["id"].as_i64())
            })
    } else if content["@type"] == "messageDocument" {
        content["document"]["thumbnail"]["file"]["id"].as_i64()
    } else {
        None
    }
}

fn image_data(file: &Value) -> Option<String> {
    let local = &file["local"];
    if local["is_downloading_completed"] != true {
        return None;
    }
    let path = local["path"].as_str()?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 512_000 {
        return None;
    }
    let mime = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

pub struct LibraryEngine {
    store: Option<LibraryStore>,
    updates: watch::Sender<LibrarySnapshot>,
    search_updates: watch::Sender<SearchSnapshot>,
    account: Option<i64>,
    selected: BTreeSet<i64>,
    queued: VecDeque<Job>,
    active: BTreeMap<i64, (Job, Instant)>,
    cover_downloads: BTreeMap<i64, (Vec<(i64, i64)>, Instant)>,
    backfill_cursor: i64,
    backfill_exhausted: bool,
    artwork_updates: Vec<(TrackKey, String)>,
    loaded: usize,
    active_initial: Option<char>,
    sort: LibrarySort,
    search_query: String,
    search_loaded: usize,
    search_pending: Option<(String, Instant)>,
    search_error: Option<String>,
    error: Option<String>,
    pause_until: Option<Instant>,
}

impl LibraryEngine {
    pub fn all_tracks(&self, sort: LibrarySort) -> Result<Vec<TrackSummary>, String> {
        let account = self.account.ok_or("No active music library")?;
        self.store
            .as_ref()
            .ok_or("Music library is unavailable")?
            .all_tracks(account, sort)
    }
    pub fn new(
        path: &str,
        updates: watch::Sender<LibrarySnapshot>,
        search_updates: watch::Sender<SearchSnapshot>,
    ) -> Self {
        let store = LibraryStore::open(Path::new(path));
        let error = store
            .as_ref()
            .err()
            .map(|_| "Can't open the music library. Check device storage and restart.".into());
        Self {
            store: store.ok(),
            updates,
            search_updates,
            account: None,
            selected: BTreeSet::new(),
            queued: VecDeque::new(),
            active: BTreeMap::new(),
            cover_downloads: BTreeMap::new(),
            backfill_cursor: 0,
            backfill_exhausted: false,
            artwork_updates: Vec::new(),
            loaded: 50,
            active_initial: None,
            sort: LibrarySort::Title,
            search_query: String::new(),
            search_loaded: 50,
            search_pending: None,
            search_error: None,
            error,
            pause_until: None,
        }
    }

    fn publish(&self) {
        let mut snapshot = LibrarySnapshot {
            account_id: self.account,
            active_initial: self.active_initial,
            sort: self.sort,
            selected_sources: self.selected.len(),
            error: self.error.clone(),
            ..Default::default()
        };
        if let (Some(store), Some(account)) = (&self.store, self.account) {
            let page = match self.sort {
                LibrarySort::Title => store.page(account, self.loaded, self.active_initial),
                LibrarySort::Recent => store.recent_page(account, self.loaded),
            };
            match page {
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
            match store.recent_page(account, HOME_RECENT_LIMIT) {
                Ok((tracks, _)) => snapshot.recent_tracks = tracks,
                Err(_) => {
                    snapshot.error =
                        Some("Can't read recent tracks. Check device storage and restart.".into())
                }
            }
            match store.recent_plays(account, HOME_RECENT_LIMIT) {
                Ok(tracks) => snapshot.recent_plays = tracks,
                Err(_) => {
                    snapshot.error = Some(
                        "Can't read listening history. Check device storage and restart.".into(),
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
        self.search_updates
            .send_replace(self.search_snapshot(&snapshot));
        self.updates.send_replace(snapshot);
    }

    pub fn refresh_recent_plays(&self) {
        self.publish();
    }

    pub fn save_embedded_art(&mut self, account: i64, chat: i64, message: i64, data: &str) {
        if self.account == Some(account)
            && self.selected.contains(&chat)
            && let Some(store) = &self.store
            && store
                .save_artwork(account, chat, message, data, 3)
                .unwrap_or(false)
        {
            self.artwork_updates.push((
                TrackKey {
                    account_id: account,
                    chat_id: chat,
                    message_id: message,
                },
                data.to_string(),
            ));
            self.publish();
        }
    }

    pub fn take_artwork_updates(&mut self) -> Vec<(TrackKey, String)> {
        std::mem::take(&mut self.artwork_updates)
    }

    fn queue_cover(&mut self, chat: i64, message: i64, file_id: i64) {
        if file_id <= 0 {
            return;
        }
        let Some(account) = self.account else {
            return;
        };
        if !self.selected.contains(&chat)
            || !self.store.as_ref().is_some_and(|store|
                store.artwork_quality(account, chat, message).is_ok_and(|quality| quality < 2))
            || self.queued.iter().any(|job| matches!(job, Job::Cover { chat: c, message: m, .. } if *c == chat && *m == message))
            || self.cover_downloads.values().any(|(keys, _)| keys.contains(&(chat, message)))
        { return; }
        self.queued.push_back(Job::Cover {
            chat,
            message,
            file_id,
        });
    }

    fn queue_backfill_page(&mut self) {
        if self.backfill_exhausted {
            return;
        }
        let (Some(account), Some(store)) = (self.account, &self.store) else {
            return;
        };
        let Ok(missing) = store.missing_artwork(account, self.backfill_cursor, BACKFILL_PAGE_SIZE)
        else {
            self.backfill_exhausted = true;
            return;
        };
        if missing.len() < BACKFILL_PAGE_SIZE {
            self.backfill_exhausted = true;
        }
        for (rowid, chat, message) in missing {
            self.backfill_cursor = rowid;
            if self.selected.contains(&chat)
                && !self.queued.iter().any(|job| matches!(job, Job::Backfill { chat: c, message: m } if *c == chat && *m == message))
            { self.queued.push_back(Job::Backfill { chat, message }); }
        }
    }

    pub fn observe_file(&mut self, value: &Value) {
        if value["@type"] != "updateFile" {
            return;
        }
        let file = &value["file"];
        let Some(file_id) = file["id"].as_i64() else {
            return;
        };
        if file["local"]["is_downloading_completed"] != true {
            return;
        }
        if let Some((keys, _)) = self.cover_downloads.remove(&file_id)
            && let (Some(account), Some(data), Some(store)) =
                (self.account, image_data(file), &self.store)
        {
            let mut changed = false;
            for (chat, message) in keys {
                if self.selected.contains(&chat)
                    && store
                        .save_artwork(account, chat, message, &data, 2)
                        .unwrap_or(false)
                {
                    changed = true;
                    self.artwork_updates.push((
                        TrackKey {
                            account_id: account,
                            chat_id: chat,
                            message_id: message,
                        },
                        data.clone(),
                    ));
                }
            }
            if changed {
                self.publish();
            }
        }
    }

    fn search_snapshot(&self, library: &LibrarySnapshot) -> SearchSnapshot {
        let mut snapshot = SearchSnapshot {
            account_id: self.account,
            query: self.search_query.clone(),
            selected_sources: library.selected_sources,
            indexing_sources: library.indexing_sources,
            recent_tracks: library.recent_tracks.clone(),
            error: self.search_error.clone(),
            indexing_error: library.error.clone(),
            ..Default::default()
        };
        if let (Some(store), Some(account)) = (&self.store, self.account) {
            match store.recent_queries(account) {
                Ok(queries) => snapshot.recent_queries = queries,
                Err(_) => snapshot.error = Some("Can't read recent searches. Try again.".into()),
            }
            if !self.search_query.is_empty() {
                match store.search_page(account, &self.search_query, self.search_loaded, 0) {
                    Ok((tracks, count)) => {
                        snapshot.has_more = count > tracks.len();
                        snapshot.total_count = count;
                        snapshot.tracks = tracks;
                    }
                    Err(_) => {
                        snapshot.error = Some("Can't search the music library. Try again.".into())
                    }
                }
            }
        }
        snapshot
    }

    pub fn reset(&mut self) {
        self.account = None;
        self.selected.clear();
        self.queued.clear();
        self.active.clear();
        self.cover_downloads.clear();
        self.backfill_cursor = 0;
        self.backfill_exhausted = false;
        self.artwork_updates.clear();
        self.pause_until = None;
        self.loaded = 50;
        self.active_initial = None;
        self.sort = LibrarySort::Title;
        self.search_query.clear();
        self.search_loaded = 50;
        self.search_pending = None;
        self.search_error = None;
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
        self.cover_downloads.retain(|_, (keys, _)| {
            keys.retain(|(chat, _)| desired.contains(chat));
            !keys.is_empty()
        });
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
        self.backfill_cursor = 0;
        self.backfill_exhausted = false;
        self.queue_backfill_page();
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
            LibraryCommand::SetSort(sort) => {
                self.sort = sort;
                self.active_initial = None;
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

    pub fn search_command(&mut self, command: SearchCommand) {
        match command {
            SearchCommand::SetQuery(query) => {
                self.search_error = None;
                let query = query
                    .chars()
                    .take(128)
                    .collect::<String>()
                    .trim()
                    .to_string();
                if query.is_empty() {
                    self.search_pending = None;
                    self.search_query = query;
                    self.search_loaded = 50;
                } else if self.search_query != query {
                    self.search_pending =
                        Some((query, Instant::now() + Duration::from_millis(180)));
                    return;
                } else {
                    self.search_pending = None;
                    return;
                }
            }
            SearchCommand::SaveQuery(query) => {
                self.search_error = None;
                let query = query
                    .chars()
                    .take(128)
                    .collect::<String>()
                    .trim()
                    .to_string();
                self.search_pending = None;
                if self.search_query != query {
                    self.search_query = query.clone();
                    self.search_loaded = 50;
                }
                if let (Some(store), Some(account)) = (&self.store, self.account)
                    && store.save_query(account, &query).is_err()
                {
                    self.search_error = Some("Can't save this recent search. Try again.".into());
                }
            }
            SearchCommand::ClearHistory => {
                self.search_error = None;
                if let (Some(store), Some(account)) = (&self.store, self.account)
                    && store.clear_history(account).is_err()
                {
                    self.search_error = Some("Can't clear recent searches. Try again.".into());
                }
            }
            SearchCommand::LoadMore => {
                self.search_loaded = self.search_loaded.saturating_add(50);
            }
            SearchCommand::Retry => {
                self.search_error = None;
                if self.error.is_some() {
                    self.command(LibraryCommand::Retry);
                }
            }
        }
        self.publish();
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
            if kind == "updateNewMessage" {
                let message = &value["message"];
                if let (Some(chat), Some(message_id), Some(file_id)) = (
                    message["chat_id"].as_i64(),
                    message["id"].as_i64(),
                    cover_file_id(message),
                ) {
                    self.queue_cover(chat, message_id, file_id);
                }
            }
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
            if let Job::Cover { file_id, .. } = job {
                self.cover_downloads.remove(&file_id);
                return true;
            }
            if let Job::Backfill { chat, message } = job {
                if value["code"] == 404
                    && let Some(store) = &self.store
                {
                    let _ = store.mark_artwork_checked(account, chat, message);
                }
                return true;
            }
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
        let mut covers = Vec::new();
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
                        .filter_map(|message| {
                            let track = parse_track(message)?;
                            if let Some(file_id) = cover_file_id(message) {
                                covers.push((chat, track.message_id, file_id));
                            }
                            Some(track)
                        })
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
                    if let Some(file_id) = cover_file_id(value) {
                        covers.push((chat, message, file_id));
                    }
                    store.upsert_live(account, chat, &track)
                } else {
                    store.remove_message(account, chat, message)
                }
            }
            Job::Backfill { chat, message } => {
                if let Some(track) = parse_track(value) {
                    if let Some(file_id) = cover_file_id(value) {
                        covers.push((chat, message, file_id));
                    } else {
                        let _ = store.mark_artwork_checked(account, chat, message);
                    }
                    store.upsert_live(account, chat, &track)
                } else {
                    store.mark_artwork_checked(account, chat, message)
                }
            }
            Job::Cover {
                chat,
                message,
                file_id,
            } => {
                if value["@type"] != "file" {
                    self.cover_downloads.remove(&file_id);
                } else if let Some(data) = image_data(value) {
                    let keys = self
                        .cover_downloads
                        .remove(&file_id)
                        .map(|(keys, _)| keys)
                        .unwrap_or_else(|| vec![(chat, message)]);
                    for (chat, message) in keys {
                        if self.selected.contains(&chat)
                            && store
                                .save_artwork(account, chat, message, &data, 2)
                                .unwrap_or(false)
                        {
                            self.artwork_updates.push((
                                TrackKey {
                                    account_id: account,
                                    chat_id: chat,
                                    message_id: message,
                                },
                                data.clone(),
                            ));
                        }
                    }
                }
                Ok(())
            }
        };
        if result.is_err() {
            self.error =
                Some("Couldn't save a music index page. Check device storage and retry.".into());
        }
        for (chat, message, file_id) in covers {
            self.queue_cover(chat, message, file_id);
        }
        self.publish();
        true
    }

    pub fn drive(&mut self, td: &TdJson) {
        if self
            .search_pending
            .as_ref()
            .is_some_and(|(_, at)| Instant::now() >= *at)
            && let Some((query, _)) = self.search_pending.take()
        {
            self.search_query = query;
            self.search_loaded = 50;
            self.publish();
        }
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
            let mut scan_timed_out = false;
            for id in timed_out {
                if let Some((job, _)) = self.active.remove(&id)
                    && matches!(job, Job::Scan { .. } | Job::Refresh { .. })
                {
                    scan_timed_out = true;
                }
            }
            if scan_timed_out {
                self.error = Some(
                    "Telegram stopped responding while indexing. Check the connection and retry."
                        .into(),
                );
                self.publish();
                return;
            }
        }
        self.cover_downloads
            .retain(|_, (_, since)| since.elapsed() <= REQUEST_TIMEOUT);
        if !self
            .queued
            .iter()
            .any(|job| matches!(job, Job::Backfill { .. }))
            && !self
                .active
                .values()
                .any(|(job, _)| matches!(job, Job::Backfill { .. }))
        {
            self.queue_backfill_page();
        }
        let mut remaining = self.queued.len();
        while self.active.len() < 2 && remaining > 0 {
            remaining -= 1;
            let Some(job) = self.queued.pop_front() else {
                break;
            };
            if !self.selected.contains(&job.chat()) {
                continue;
            }
            if let Job::Cover {
                chat,
                message,
                file_id,
            } = job
            {
                if let Some((keys, _)) = self.cover_downloads.get_mut(&file_id) {
                    if !keys.contains(&(chat, message)) {
                        keys.push((chat, message));
                    }
                    continue;
                }
                if self.cover_downloads.len() >= 2 {
                    self.queued.push_back(job);
                    continue;
                }
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
                Job::Refresh { chat, message } | Job::Backfill { chat, message } => json!({
                    "@type":"getMessage", "chat_id":chat, "message_id":message
                }),
                Job::Cover { file_id, .. } => json!({
                    "@type":"downloadFile", "file_id":file_id, "priority":1,
                    "offset":0, "limit":0, "synchronous":false
                }),
            };
            let extra = td.next_request_id();
            let mut request = request;
            request["@extra"] = json!(extra);
            if td.send(&request).is_ok() {
                if let Job::Cover {
                    chat,
                    message,
                    file_id,
                } = job
                {
                    self.cover_downloads
                        .insert(file_id, (vec![(chat, message)], Instant::now()));
                }
                self.active.insert(extra, (job, Instant::now()));
            } else {
                let artwork_only = matches!(job, Job::Cover { .. } | Job::Backfill { .. });
                self.queued.push_front(job);
                if !artwork_only {
                    self.error = Some(
                        "Couldn't request Telegram music. Check the connection and retry.".into(),
                    );
                }
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
            "album_cover_minithumbnail":{"data":"abc"},
            "album_cover_thumbnail":{"file":{"id":44}}
        }}});
        let track = parse_track(&audio).unwrap();
        assert_eq!(track.title, "Persian");
        assert_eq!(track.artist, "Artist");
        assert_eq!(track.cover_data.as_deref(), Some("abc"));
        assert_eq!(cover_file_id(&audio), Some(44));
        let document = json!({"id":8,"date":11,"content":{"@type":"messageDocument","document":{
            "mime_type":"audio/flac","file_name":"آهنگ.flac",
            "minithumbnail":{"data":"mini"},"thumbnail":{"file":{"id":55}}
        }}});
        let track = parse_track(&document).unwrap();
        assert_eq!(track.title, "آهنگ");
        assert_eq!(track.artist, "Unknown artist");
        assert_eq!(track.media_type, "document");
        assert_eq!(track.cover_data.as_deref(), Some("mini"));
        assert_eq!(cover_file_id(&document), Some(55));
        let external = json!({"content":{"@type":"messageAudio","audio":{
            "external_album_covers":[{"file":{"id":66}}]
        }}});
        assert_eq!(cover_file_id(&external), Some(66));
        assert!(parse_track(&json!({"id":9,"content":{"@type":"messageVoiceNote"}})).is_none());
    }

    #[test]
    fn thumbnail_completion_updates_library_and_survives_restart() {
        let root = std::env::temp_dir().join(format!(
            "tunestash-cover-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("library.sqlite");
        let source = SourceStore::open(&db).unwrap();
        let chat = SourceChat {
            chat_id: 42,
            title: "Music".into(),
            subtitle: String::new(),
            kind: SourceKind::OtherChat,
            selected: true,
            music: MusicCheck::Found,
        };
        source.set_selected(7, &chat, true).unwrap();
        let mut store = LibraryStore::open(&db).unwrap();
        store.start_source(7, 42, false).unwrap();
        let track = parse_track(&json!({"id":9,"date":1,"content":{"@type":"messageAudio",
            "audio":{"file_name":"nine.mp3"}}}))
        .unwrap();
        store
            .save_page(7, 42, MediaFilter::Audio, 0, &[track])
            .unwrap();
        store
            .save_page(7, 42, MediaFilter::Document, 0, &[])
            .unwrap();
        drop(store);
        let (updates, receiver) = watch::channel(LibrarySnapshot::default());
        let (search_updates, _) = watch::channel(SearchSnapshot::default());
        let mut engine = LibraryEngine::new(db.to_str().unwrap(), updates, search_updates);
        engine.sync_sources(&SourceSnapshot {
            account_id: Some(7),
            setup_complete: true,
            chats: vec![chat],
            ..Default::default()
        });
        engine.queue_cover(42, 9, 88);
        engine.drive(&TdJson);
        let extra = *engine
            .active
            .iter()
            .find(|(_, (job, _))| matches!(job, Job::Cover { file_id: 88, .. }))
            .unwrap()
            .0;
        assert!(
            engine.handle_response(&json!({"@extra":extra,"@type":"file","id":88,
            "local":{"is_downloading_completed":false}}))
        );
        let png = root.join("cover.png");
        std::fs::write(&png, STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=").unwrap()).unwrap();
        engine.observe_file(&json!({"@type":"updateFile","file":{"id":88,
            "local":{"is_downloading_completed":true,"path":png}}}));
        assert!(
            receiver.borrow().tracks[0]
                .cover_data
                .as_ref()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        drop(engine);
        let reopened = LibraryStore::open(&db).unwrap();
        assert_eq!(reopened.artwork_quality(7, 42, 9).unwrap(), 2);
        assert!(
            reopened.page(7, 10, None).unwrap().0[0]
                .cover_data
                .is_some()
        );
        drop(reopened);
        drop(source);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backfill_pages_reach_tracks_beyond_the_first_batch() {
        let root =
            std::env::temp_dir().join(format!("tunestash-cover-backfill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("library.sqlite");
        let source = SourceStore::open(&db).unwrap();
        let chat = SourceChat {
            chat_id: 42,
            title: "Music".into(),
            subtitle: String::new(),
            kind: SourceKind::OtherChat,
            selected: true,
            music: MusicCheck::Found,
        };
        source.set_selected(7, &chat, true).unwrap();
        let mut store = LibraryStore::open(&db).unwrap();
        store.start_source(7, 42, false).unwrap();
        let tracks: Vec<_> = (1..=201)
            .map(|id| TrackInput {
                message_id: id,
                date: id,
                media_type: "audio",
                title: format!("Song {id}"),
                artist: "Artist".into(),
                filename: format!("{id}.mp3"),
                duration_seconds: None,
                cover_data: None,
            })
            .collect();
        store
            .save_page(7, 42, MediaFilter::Audio, 0, &tracks)
            .unwrap();
        store
            .save_page(7, 42, MediaFilter::Document, 0, &[])
            .unwrap();
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute("UPDATE indexed_tracks SET cover_checked=0", [])
            .unwrap();
        drop(store);
        let (updates, _) = watch::channel(LibrarySnapshot::default());
        let (search_updates, _) = watch::channel(SearchSnapshot::default());
        let mut engine = LibraryEngine::new(db.to_str().unwrap(), updates, search_updates);
        engine.sync_sources(&SourceSnapshot {
            account_id: Some(7),
            setup_complete: true,
            chats: vec![chat],
            ..Default::default()
        });
        assert_eq!(engine.queued.len(), BACKFILL_PAGE_SIZE);
        engine.queued.clear();
        engine.queue_backfill_page();
        assert_eq!(engine.queued.len(), 1);
        assert!(matches!(
            engine.queued.front(),
            Some(Job::Backfill { message: 201, .. })
        ));
        drop(engine);
        drop(source);
        std::fs::remove_dir_all(root).unwrap();
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
        let (search_updates, _) = watch::channel(SearchSnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates, search_updates);
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
        let (search_updates, _) = watch::channel(SearchSnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates, search_updates);
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
        let (search_updates, search_receiver) = watch::channel(SearchSnapshot::default());
        let mut engine = LibraryEngine::new(path.to_str().unwrap(), updates, search_updates);
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
        assert_eq!(
            receiver
                .borrow()
                .recent_tracks
                .iter()
                .map(|track| track.message_id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        engine.command(LibraryCommand::SetSort(LibrarySort::Recent));
        assert_eq!(receiver.borrow().sort, LibrarySort::Recent);
        assert_eq!(receiver.borrow().active_initial, None);
        assert_eq!(receiver.borrow().tracks[0].message_id, 3);
        engine.command(LibraryCommand::SetSort(LibrarySort::Title));
        assert_eq!(receiver.borrow().sort, LibrarySort::Title);
        assert_eq!(receiver.borrow().tracks[0].title, "Amber");
        engine.command(LibraryCommand::SelectInitial('Z'));
        assert_eq!(receiver.borrow().active_initial, Some('Z'));
        assert_eq!(receiver.borrow().total_count, 0);
        engine.command(LibraryCommand::SelectInitial('Z'));
        assert_eq!(receiver.borrow().active_initial, None);
        assert_eq!(receiver.borrow().total_count, 3);
        engine.command(LibraryCommand::SelectInitial('1'));
        assert_eq!(receiver.borrow().active_initial, None);
        engine.command(LibraryCommand::SelectInitial('A'));
        engine.search_command(SearchCommand::SaveQuery("Blue".into()));
        assert_eq!(receiver.borrow().active_initial, Some('A'));
        assert_eq!(search_receiver.borrow().total_count, 1);
        assert_eq!(search_receiver.borrow().tracks[0].title, "Blue");
        engine.observe_update(&json!({
            "@type":"updateNewMessage", "message":{
                "id":4, "chat_id":42, "date":4,
                "content":{"@type":"messageAudio", "audio":{
                    "title":"Bluebird", "file_name":"bluebird.mp3"
                }}
            }
        }));
        assert_eq!(search_receiver.borrow().total_count, 2);
        assert_eq!(receiver.borrow().total_count, 2);
        engine.search_command(SearchCommand::SetQuery("Amber".into()));
        assert!(engine.search_pending.is_some());
        assert_eq!(search_receiver.borrow().recent_queries, vec!["Blue"]);
        drop(engine);
        drop(source_store);
        std::fs::remove_file(path).unwrap();
    }
}

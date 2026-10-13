//! Playback orchestration on the TDLib worker. File IDs never enter durable state.

use super::{TdJson, library::LibraryEngine};
use crate::adapters::{media, sqlite::playback::PlaybackStore};
use music_core::domain::{
    DownloadStatus, LibrarySort, PlaybackCommand, PlaybackSnapshot, PlaybackStatus, QueueTrack,
    TrackKey,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

struct FileState {
    file_id: i64,
    size: i64,
    path: String,
    offset: i64,
    prefix: i64,
    complete: bool,
    copying: bool,
}

struct PendingRead {
    key: TrackKey,
    offset: i64,
    length: usize,
    response: Sender<Result<Vec<u8>, String>>,
    deadline: Instant,
}

type CopyResult = (TrackKey, Result<(PathBuf, u64), String>);

pub struct PlaybackEngine {
    account: Option<i64>,
    state: PlaybackSnapshot,
    updates: watch::Sender<PlaybackSnapshot>,
    store: Option<PlaybackStore>,
    media_root: PathBuf,
    files: HashMap<TrackKey, FileState>,
    resolving: HashMap<i64, TrackKey>,
    reads: Vec<PendingRead>,
    copy_tx: Sender<CopyResult>,
    copy_rx: Receiver<CopyResult>,
    last_persist: Instant,
    last_reconcile: Instant,
    fallback_key: Option<TrackKey>,
    recover_downloads: Vec<TrackKey>,
}

impl PlaybackEngine {
    pub fn new(db: &str, files_dir: &str, updates: watch::Sender<PlaybackSnapshot>) -> Self {
        let (copy_tx, copy_rx) = mpsc::channel();
        Self {
            account: None,
            state: PlaybackSnapshot::default(),
            updates,
            store: PlaybackStore::open(Path::new(db)).ok(),
            media_root: Path::new(files_dir)
                .parent()
                .unwrap_or(Path::new(files_dir))
                .join("offline"),
            files: HashMap::new(),
            resolving: HashMap::new(),
            reads: Vec::new(),
            copy_tx,
            copy_rx,
            last_persist: Instant::now(),
            last_reconcile: Instant::now(),
            fallback_key: None,
            recover_downloads: Vec::new(),
        }
    }

    fn publish(&mut self) {
        self.updates.send_replace(self.state.clone());
    }

    fn persist(&mut self) {
        if let Some(store) = &mut self.store {
            let _ = store.persist(&self.state);
        }
        self.last_persist = Instant::now();
    }

    pub fn set_account(&mut self, account: Option<i64>) {
        if self.account == account {
            return;
        }
        self.account = account;
        self.recover_downloads = account
            .and_then(|id| self.store.as_ref()?.pending_downloads(id).ok())
            .unwrap_or_default();
        self.files.clear();
        self.resolving.clear();
        self.fallback_key = None;
        for read in self.reads.drain(..) {
            let _ = read.response.send(Err("Account changed".into()));
        }
        self.state = account
            .and_then(|id| self.store.as_ref()?.restore(id).ok())
            .unwrap_or_default();
        self.refresh_current_flags();
        self.publish();
    }

    pub fn logout(&mut self, account: i64) {
        let _ = media::dispatch("stop", "{}");
        self.set_account(None);
        let _ = std::fs::remove_dir_all(self.media_root.join(account.to_string()));
    }

    fn refresh_current_flags(&mut self) {
        if let Some(key) = self.state.current_key() {
            self.state.favorite = self
                .store
                .as_ref()
                .and_then(|s| s.favorite(key).ok())
                .unwrap_or(false);
            self.state.download = if self
                .store
                .as_ref()
                .and_then(|s| s.saved_path(key).ok().flatten())
                .is_some()
            {
                DownloadStatus::Saved
            } else {
                match self
                    .store
                    .as_ref()
                    .and_then(|s| s.download_status(key).ok().flatten())
                    .as_deref()
                {
                    Some("queued" | "downloading") => DownloadStatus::Downloading,
                    Some("failed") => DownloadStatus::Failed,
                    _ => DownloadStatus::Remote,
                }
            };
        } else {
            self.state.favorite = false;
            self.state.download = DownloadStatus::Remote;
        }
    }

    fn prepare(&mut self, td: &TdJson) {
        self.prepare_at(td, 0);
    }

    fn prepare_at(&mut self, td: &TdJson, position_ms: i64) {
        let Some(track) = self.state.current().cloned() else {
            return;
        };
        self.state.status = PlaybackStatus::Loading;
        self.state.position_ms = position_ms.max(0);
        self.state.artwork_data = None;
        self.state.error = None;
        self.fallback_key = None;
        self.refresh_current_flags();
        let key = track.key;
        let saved = self
            .store
            .as_ref()
            .and_then(|s| s.saved_path(key).ok().flatten());
        let uri = if let Some(path) = saved {
            format!("file://{path}")
        } else {
            self.ensure_file(td, key);
            format!(
                "tunestash://{}/{}/{}",
                key.account_id, key.chat_id, key.message_id
            )
        };
        let payload = json!({"uri":uri,"title":track.title,"artist":track.artist,
            "duration":track.duration_seconds.unwrap_or(0) * 1000,
            "media_id":format!("{}:{}:{}",key.account_id,key.chat_id,key.message_id),
            "position":self.state.position_ms});
        if let Err(error) = media::dispatch("play", &payload.to_string()) {
            self.state.status = PlaybackStatus::Failed;
            self.state.error = Some(format!("Cannot start Android playback: {error}"));
        }
        self.persist();
        self.publish();
    }

    pub fn command(&mut self, td: &TdJson, library: &LibraryEngine, command: PlaybackCommand) {
        match command {
            PlaybackCommand::PlayLibrary { key, sort } => {
                if self.account != Some(key.account_id) {
                    return;
                }
                match library.all_tracks(sort) {
                    Ok(tracks) => {
                        let was_shuffled = self.state.shuffle;
                        let queue: Vec<_> = tracks
                            .into_iter()
                            .map(|t| QueueTrack {
                                key: TrackKey {
                                    account_id: key.account_id,
                                    chat_id: t.chat_id,
                                    message_id: t.message_id,
                                },
                                title: t.title,
                                artist: t.artist,
                                filename: t.filename,
                                duration_seconds: t.duration_seconds,
                                cover_data: t.cover_data,
                            })
                            .collect();
                        if self.state.load_queue(queue, key) {
                            if was_shuffled {
                                self.state.shuffle = false;
                                self.state.toggle_shuffle(
                                    SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_nanos() as u64,
                                );
                            }
                            self.prepare(td);
                        }
                    }
                    Err(error) => {
                        self.state.error = Some(error);
                        self.state.status = PlaybackStatus::Failed;
                        self.publish();
                    }
                }
            }
            PlaybackCommand::Play => {
                if self.state.current_key().is_some() {
                    let position = if self
                        .state
                        .duration_ms
                        .is_some_and(|duration| self.state.position_ms >= duration)
                    {
                        0
                    } else {
                        self.state.position_ms
                    };
                    self.prepare_at(td, position);
                }
            }
            PlaybackCommand::Pause => {
                let _ = media::dispatch("pause", "{}");
            }
            PlaybackCommand::Previous => {
                if self.state.position_ms > 3_000 || self.state.cursor == 0 {
                    let _ = media::dispatch("seek", "{\"position\":0}");
                    self.state.position_ms = 0;
                } else {
                    self.state.cursor -= 1;
                    self.prepare(td);
                }
            }
            PlaybackCommand::Next => {
                if self.state.advance(false).is_some() {
                    self.prepare(td);
                } else {
                    let _ = media::dispatch("stop", "{}");
                    self.state.status = PlaybackStatus::Paused;
                    self.publish();
                }
            }
            PlaybackCommand::Select(key) => {
                if self.state.select(key) {
                    self.prepare(td);
                }
            }
            PlaybackCommand::Seek(ms) => {
                let ms = ms.max(0).min(self.state.duration_ms.unwrap_or(i64::MAX));
                let _ = media::dispatch("seek", &json!({"position":ms}).to_string());
                self.state.position_ms = ms;
            }
            PlaybackCommand::ToggleShuffle => {
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                self.state.toggle_shuffle(seed);
            }
            PlaybackCommand::CycleRepeat => {
                self.state.repeat = self.state.repeat.next();
            }
            PlaybackCommand::ToggleFavorite => {
                if let (Some(store), Some(key)) = (&self.store, self.state.current_key())
                    && let Ok(favorite) = store.toggle_favorite(key)
                {
                    self.state.favorite = favorite;
                }
            }
            PlaybackCommand::Download => {
                if let Some(key) = self.state.current_key() {
                    if self
                        .store
                        .as_ref()
                        .and_then(|s| s.saved_path(key).ok().flatten())
                        .is_some()
                    {
                        return;
                    }
                    if let Some(file) = self.files.get_mut(&key) {
                        file.copying = false;
                    }
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "queued", None);
                    }
                    self.state.download = DownloadStatus::Downloading;
                    self.ensure_file(td, key);
                    if let Some(file) = self.files.get(&key) {
                        let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,"priority":24,"offset":0,"limit":0,"synchronous":false}));
                    }
                }
            }
            PlaybackCommand::DownloadTrack(key) => {
                if self.account == Some(key.account_id)
                    && self
                        .store
                        .as_ref()
                        .and_then(|s| s.saved_path(key).ok().flatten())
                        .is_none()
                {
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "queued", None);
                    }
                    self.ensure_file(td, key);
                    if let Some(file) = self.files.get(&key) {
                        let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,
                            "priority":24,"offset":0,"limit":0,"synchronous":false}));
                    }
                }
            }
        }
        self.persist();
        self.publish();
    }

    fn ensure_file(&mut self, td: &TdJson, key: TrackKey) {
        if self.files.contains_key(&key)
            || self.resolving.values().any(|candidate| *candidate == key)
        {
            return;
        }
        if let Some(store) = &self.store {
            let _ = store.mark_download(key, "queued", None);
        }
        let extra = td.next_request_id();
        self.resolving.insert(extra, key);
        let _ = td.send(&json!({"@type":"getMessage","chat_id":key.chat_id,"message_id":key.message_id,"@extra":extra}));
        if self.state.current_key() == Some(key) {
            self.state.download = DownloadStatus::Downloading;
        }
    }

    pub fn read_range(
        &mut self,
        td: &TdJson,
        key: TrackKey,
        offset: i64,
        length: usize,
        response: Sender<Result<Vec<u8>, String>>,
    ) {
        if self.account != Some(key.account_id) || offset < 0 || length == 0 {
            let _ = response.send(Err("Invalid audio range".into()));
            return;
        }
        if let Some(path) = self
            .store
            .as_ref()
            .and_then(|s| s.saved_path(key).ok().flatten())
        {
            let _ = response.send(read_file(&path, offset, length));
            return;
        }
        self.ensure_file(td, key);
        self.reads.push(PendingRead {
            key,
            offset,
            length,
            response,
            deadline: Instant::now() + Duration::from_secs(25),
        });
        if let Some(file) = self.files.get(&key) {
            let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,
                "priority":32,"offset":offset,"limit":0,"synchronous":false}));
        }
        self.flush_reads(td);
    }

    pub fn handle_value(&mut self, td: &TdJson, value: &Value) -> bool {
        if let Some(extra) = value["@extra"].as_i64()
            && let Some(key) = self.resolving.remove(&extra)
        {
            if value["@type"] == "message" {
                let content = &value["content"];
                let file = if content["@type"] == "messageAudio" {
                    &content["audio"]["audio"]
                } else {
                    &content["document"]["document"]
                };
                self.update_file(key, file);
                if let Some(file) = self.files.get(&key) {
                    let offset = self
                        .reads
                        .iter()
                        .find(|read| read.key == key)
                        .map_or(0, |read| read.offset);
                    let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,
                        "priority":24,"offset":offset,"limit":0,"synchronous":false}));
                }
            } else {
                if let Some(store) = &self.store {
                    let _ =
                        store.mark_download(key, "failed", Some("Telegram audio is unavailable"));
                }
                if self.state.current_key() == Some(key) {
                    self.state.error = Some(
                        "This Telegram audio is no longer accessible. Try another track.".into(),
                    );
                    self.state.status = PlaybackStatus::Failed;
                    self.state.download = DownloadStatus::Failed;
                    self.publish();
                }
            }
            self.flush_reads(td);
            return true;
        }
        let file = if value["@type"] == "updateFile" {
            &value["file"]
        } else if value["@type"] == "file" {
            value
        } else {
            return false;
        };
        let Some(id) = file["id"].as_i64() else {
            return false;
        };
        let Some(key) = self
            .files
            .iter()
            .find_map(|(key, state)| (state.file_id == id).then_some(*key))
        else {
            return false;
        };
        self.update_file(key, file);
        self.flush_reads(td);
        true
    }

    fn update_file(&mut self, key: TrackKey, file: &Value) {
        let Some(id) = file["id"].as_i64() else {
            return;
        };
        let local = &file["local"];
        let state = self.files.entry(key).or_insert(FileState {
            file_id: id,
            size: 0,
            path: String::new(),
            offset: 0,
            prefix: 0,
            complete: false,
            copying: false,
        });
        state.file_id = id;
        state.size = file["size"]
            .as_i64()
            .filter(|size| *size > 0)
            .or_else(|| file["expected_size"].as_i64())
            .unwrap_or(state.size);
        state.path = local["path"].as_str().unwrap_or(&state.path).to_string();
        state.offset = local["download_offset"].as_i64().unwrap_or(state.offset);
        state.prefix = local["downloaded_prefix_size"]
            .as_i64()
            .unwrap_or(state.prefix);
        state.complete = local["is_downloading_completed"] == true;
        if !state.complete
            && let Some(store) = &self.store
        {
            let _ = store.mark_download(key, "downloading", None);
        }
        if state.complete && !state.copying && !state.path.is_empty() {
            state.copying = true;
            let source = state.path.clone();
            let expected = state.size;
            let dest = self
                .media_root
                .join(key.account_id.to_string())
                .join(format!("{}-{}.audio", key.chat_id, key.message_id));
            let tx = self.copy_tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send((key, copy_complete(&source, &dest, expected)));
            });
        }
    }

    fn flush_reads(&mut self, td: &TdJson) {
        let mut pending = Vec::new();
        for read in self.reads.drain(..) {
            let Some(file) = self.files.get(&read.key) else {
                if read.deadline > Instant::now() {
                    pending.push(read);
                } else {
                    let _ = read
                        .response
                        .send(Err("Telegram did not return the audio file".into()));
                }
                continue;
            };
            if file.size > 0 && read.offset >= file.size {
                let _ = read.response.send(Ok(Vec::new()));
                continue;
            }
            if file.complete
                || (read.offset >= file.offset && read.offset < file.offset + file.prefix)
            {
                let available = if file.complete {
                    read.length
                } else {
                    read.length
                        .min((file.offset + file.prefix - read.offset) as usize)
                };
                let _ = read
                    .response
                    .send(read_file(&file.path, read.offset, available));
                if !file.complete {
                    let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,
                        "priority":16,"offset":0,"limit":0,"synchronous":false}));
                }
            } else if read.deadline <= Instant::now() {
                let _ = read.response.send(Err(
                    "Audio buffering timed out. Check your connection.".into(),
                ));
            } else {
                pending.push(read);
            }
        }
        self.reads = pending;
    }

    pub fn media_event(&mut self, td: &TdJson, value: &Value) {
        let mut persist = true;
        match value["type"].as_str().unwrap_or("") {
            "playing" => self.state.status = PlaybackStatus::Playing,
            "paused" => self.state.status = PlaybackStatus::Paused,
            "buffering" => self.state.status = PlaybackStatus::Buffering,
            "ready" => {
                self.state.duration_ms = value["duration"].as_i64().filter(|v| *v > 0);
                if self.state.status == PlaybackStatus::Loading {
                    self.state.status = PlaybackStatus::Buffering;
                }
            }
            "position" => {
                self.state.position_ms = value["position"]
                    .as_i64()
                    .unwrap_or(self.state.position_ms)
                    .max(0);
                persist = self.last_persist.elapsed() > Duration::from_secs(5);
            }
            "artwork" => {
                let expected = self
                    .state
                    .current_key()
                    .map(|key| format!("{}:{}:{}", key.account_id, key.chat_id, key.message_id));
                if expected.as_deref() != value["media_id"].as_str() {
                    return;
                }
                let Some(data) = value["data"].as_str().filter(|data| data.len() < 700_000) else {
                    return;
                };
                self.state.artwork_data = Some(data.to_string());
                persist = false;
            }
            "ended" => {
                if self.state.advance(true).is_some() {
                    self.prepare(td);
                } else {
                    self.state.status = PlaybackStatus::Paused;
                }
            }
            "next" => self.command_without_library(td, true),
            "previous" => self.command_without_library(td, false),
            "error" => {
                self.state.status = PlaybackStatus::Failed;
                self.state.error = Some(
                    value["message"]
                        .as_str()
                        .unwrap_or("This audio cannot be played.")
                        .to_string(),
                );
                if self.state.download == DownloadStatus::Saved {
                    if let (Some(store), Some(key)) = (&self.store, self.state.current_key()) {
                        let _ = store.invalidate_saved(key);
                    }
                    self.state.download = DownloadStatus::Failed;
                } else {
                    self.fallback_key = self.state.current_key();
                }
            }
            _ => return,
        }
        if persist {
            self.persist();
        }
        self.publish();
    }

    fn command_without_library(&mut self, td: &TdJson, next: bool) {
        if next {
            if self.state.advance(false).is_some() {
                self.prepare(td);
            }
        } else if self.state.cursor > 0 {
            self.state.cursor -= 1;
            self.prepare(td);
        }
    }

    pub fn tick(&mut self, td: &TdJson) {
        for key in std::mem::take(&mut self.recover_downloads) {
            self.ensure_file(td, key);
        }
        self.flush_reads(td);
        while let Ok((key, result)) = self.copy_rx.try_recv() {
            match result {
                Ok((path, size)) => {
                    if self.account != Some(key.account_id) {
                        let _ = std::fs::remove_file(path);
                        continue;
                    }
                    let saved = self
                        .store
                        .as_ref()
                        .is_some_and(|store| store.save_file(key, &path, size).is_ok());
                    if saved {
                        if self.state.current_key() == Some(key) {
                            self.state.download = DownloadStatus::Saved;
                            self.publish();
                        }
                        if self.fallback_key == Some(key) {
                            self.fallback_key = None;
                            self.prepare(td);
                        }
                    } else {
                        let _ = std::fs::remove_file(path);
                        if self.state.current_key() == Some(key) {
                            self.state.download = DownloadStatus::Failed;
                            self.state.error = Some(
                                "Could not record the saved audio. Check device storage.".into(),
                            );
                            self.publish();
                        }
                    }
                }
                Err(error) if self.state.current_key() == Some(key) => {
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "failed", Some(&error));
                    }
                    self.state.download = DownloadStatus::Failed;
                    self.state.error = Some(format!("Could not save this audio: {error}"));
                    self.publish();
                }
                Err(error) => {
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "failed", Some(&error));
                    }
                }
            }
        }
    }

    pub fn reconcile(&mut self, library: &LibraryEngine) {
        if self.state.queue.is_empty() || self.last_reconcile.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_reconcile = Instant::now();
        let Ok(tracks) = library.all_tracks(LibrarySort::Title) else {
            return;
        };
        let available: HashSet<_> = tracks
            .iter()
            .map(|track| TrackKey {
                account_id: self.account.unwrap_or_default(),
                chat_id: track.chat_id,
                message_id: track.message_id,
            })
            .collect();
        let count = self.state.queue.len();
        let removed_current = self.state.retain_available(&available);
        if count == self.state.queue.len() {
            return;
        }
        if removed_current {
            let _ = media::dispatch("stop", "{}");
            self.state.status = PlaybackStatus::Paused;
            self.state.error =
                Some("The selected source is no longer available. Choose another track.".into());
            self.refresh_current_flags();
        }
        self.persist();
        self.publish();
    }
}

fn read_file(path: &str, offset: i64, length: usize) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(offset as u64))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![0; length];
    let count = file.read(&mut bytes).map_err(|e| e.to_string())?;
    bytes.truncate(count);
    Ok(bytes)
}

fn copy_complete(source: &str, dest: &Path, expected: i64) -> Result<(PathBuf, u64), String> {
    let meta = std::fs::metadata(source).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() == 0 {
        return Err("Telegram returned an empty file".into());
    }
    if expected > 0 && meta.len() != expected as u64 {
        return Err("Telegram file size did not match the completed download".into());
    }
    std::fs::create_dir_all(dest.parent().ok_or("Invalid offline path")?).map_err(storage_error)?;
    let part = dest.with_extension("part");
    let copied = std::fs::copy(source, &part).map_err(storage_error)?;
    if copied != meta.len() {
        let _ = std::fs::remove_file(&part);
        return Err("Incomplete local copy".into());
    }
    std::fs::File::open(&part)
        .and_then(|file| file.sync_all())
        .map_err(storage_error)?;
    std::fs::rename(&part, dest).map_err(storage_error)?;
    Ok((dest.to_path_buf(), copied))
}

fn storage_error(error: std::io::Error) -> String {
    if error.raw_os_error() == Some(28) {
        "Device storage is full. Free space and retry.".into()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_copy_never_becomes_a_saved_file() {
        let dir = std::env::temp_dir().join(format!("tunestash-copy-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source");
        let dest = dir.join("saved");
        std::fs::write(&source, b"abc").unwrap();
        assert!(copy_complete(source.to_str().unwrap(), &dest, 4).is_err());
        assert!(!dest.exists());
        assert_eq!(
            copy_complete(source.to_str().unwrap(), &dest, 3).unwrap().1,
            3
        );
        assert_eq!(std::fs::read(dest).unwrap(), b"abc");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn byte_reads_respect_the_requested_range_and_eof() {
        let path =
            std::env::temp_dir().join(format!("tunestash-range-test-{}", std::process::id()));
        std::fs::write(&path, b"abcdefgh").unwrap();
        assert_eq!(read_file(path.to_str().unwrap(), 3, 3).unwrap(), b"def");
        assert_eq!(read_file(path.to_str().unwrap(), 7, 4).unwrap(), b"h");
        assert!(read_file(path.to_str().unwrap(), 8, 4).unwrap().is_empty());
        let _ = std::fs::remove_file(path);
    }
}

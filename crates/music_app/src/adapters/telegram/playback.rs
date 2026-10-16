//! Playback orchestration on the TDLib worker. File IDs never enter durable state.

use super::{TdJson, library::LibraryEngine};
use crate::adapters::{media, sqlite::playback::PlaybackStore};
use music_core::domain::{
    DownloadStatus, LibrarySort, PlaybackCommand, PlaybackSnapshot, PlaybackStatus, QueueTrack,
    StorageCommand, StorageSnapshot, TrackKey,
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

type CopyResult = (TrackKey, u64, Result<(PathBuf, u64), String>);
type ScanResult = (i64, u64, Result<StorageSnapshot, String>);

pub struct PlaybackEngine {
    account: Option<i64>,
    state: PlaybackSnapshot,
    updates: watch::Sender<PlaybackSnapshot>,
    store: Option<PlaybackStore>,
    media_root: PathBuf,
    cache_root: PathBuf,
    data_root: PathBuf,
    db_path: PathBuf,
    storage: StorageSnapshot,
    storage_updates: watch::Sender<StorageSnapshot>,
    scan_tx: Sender<ScanResult>,
    scan_rx: Receiver<ScanResult>,
    scan_generation: u64,
    visible_limit: usize,
    next_epoch: u64,
    epochs: HashMap<TrackKey, u64>,
    cache_deletes: Vec<(i64, Instant)>,
    cache_clear_extra: Option<i64>,
    cache_cleanup_again: bool,
    startup_cleanup_pending: bool,
    files: HashMap<TrackKey, FileState>,
    resolving: HashMap<i64, TrackKey>,
    reads: Vec<PendingRead>,
    copy_tx: Sender<CopyResult>,
    copy_rx: Receiver<CopyResult>,
    last_persist: Instant,
    last_reconcile: Instant,
    fallback_key: Option<TrackKey>,
    fallback_attempted: HashSet<TrackKey>,
    recover_downloads: Vec<TrackKey>,
    last_recorded_key: Option<TrackKey>,
    copy_workers: Vec<std::thread::JoinHandle<()>>,
}

impl PlaybackEngine {
    pub fn new(
        db: &str,
        files_dir: &str,
        updates: watch::Sender<PlaybackSnapshot>,
        storage_updates: watch::Sender<StorageSnapshot>,
    ) -> Self {
        let (copy_tx, copy_rx) = mpsc::channel();
        let (scan_tx, scan_rx) = mpsc::channel();
        let cache_root = Path::new(files_dir).to_path_buf();
        let data_root = cache_root
            .ancestors()
            .nth(3)
            .unwrap_or(Path::new(files_dir))
            .to_path_buf();
        Self {
            account: None,
            state: PlaybackSnapshot::default(),
            updates,
            store: PlaybackStore::open(Path::new(db)).ok(),
            media_root: Path::new(files_dir)
                .parent()
                .unwrap_or(Path::new(files_dir))
                .join("offline"),
            cache_root,
            data_root,
            db_path: Path::new(db).to_path_buf(),
            storage: StorageSnapshot::default(),
            storage_updates,
            scan_tx,
            scan_rx,
            scan_generation: 0,
            visible_limit: 50,
            next_epoch: 0,
            epochs: HashMap::new(),
            cache_deletes: Vec::new(),
            cache_clear_extra: None,
            cache_cleanup_again: false,
            startup_cleanup_pending: false,
            files: HashMap::new(),
            resolving: HashMap::new(),
            reads: Vec::new(),
            copy_tx,
            copy_rx,
            last_persist: Instant::now(),
            last_reconcile: Instant::now(),
            fallback_key: None,
            fallback_attempted: HashSet::new(),
            recover_downloads: Vec::new(),
            last_recorded_key: None,
            copy_workers: Vec::new(),
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
        self.last_recorded_key = None;
        self.recover_downloads = account
            .and_then(|id| self.store.as_ref()?.pending_downloads(id).ok())
            .unwrap_or_default();
        self.files.clear();
        self.resolving.clear();
        self.epochs.clear();
        self.cache_deletes.clear();
        self.cache_clear_extra = None;
        self.cache_cleanup_again = false;
        self.visible_limit = 50;
        self.fallback_key = None;
        self.fallback_attempted.clear();
        for read in self.reads.drain(..) {
            let _ = read.response.send(Err("Account changed".into()));
        }
        self.state = account
            .and_then(|id| self.store.as_ref()?.restore(id).ok())
            .unwrap_or_default();
        self.refresh_current_flags();
        self.publish();
        self.storage = StorageSnapshot {
            account_id: account,
            ..StorageSnapshot::default()
        };
        self.storage.stream_only = account
            .and_then(|id| self.store.as_ref()?.stream_only(id).ok())
            .unwrap_or(false);
        if self.storage.stream_only {
            self.recover_downloads.clear();
            if let (Some(store), Some(id)) = (&self.store, account) {
                let _ = store.clear_pending(id);
            }
        }
        self.startup_cleanup_pending = self.storage.stream_only;
        self.storage_updates.send_replace(self.storage.clone());
        self.refresh_storage();
    }

    pub fn begin_logout(&mut self, td: &TdJson) {
        for file in self.files.values() {
            let _ = td.send(&json!({"@type":"cancelDownloadFile","file_id":file.file_id,
                "only_if_pending":false}));
        }
        let _ = media::dispatch("stop", "{}");
        self.set_account(None);
        for worker in self.copy_workers.drain(..) {
            let _ = worker.join();
        }
    }

    pub fn finish_logout(&mut self, account: i64) -> Result<(), String> {
        match std::fs::remove_dir_all(self.media_root.join(account.to_string())) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("Could not remove offline songs: {error}")),
        }
    }

    fn publish_storage(&mut self) {
        self.storage_updates.send_replace(self.storage.clone());
    }

    fn refresh_storage(&mut self) {
        let Some(account) = self.account else {
            return;
        };
        self.storage.busy = true;
        self.publish_storage();
        self.scan_generation = self.scan_generation.wrapping_add(1);
        let generation = self.scan_generation;
        let tx = self.scan_tx.clone();
        let db = self.db_path.clone();
        let data = self.data_root.clone();
        let cache = self.cache_root.clone();
        let limit = self.visible_limit;
        std::thread::spawn(move || {
            let result = measure_storage(&db, &data, &cache, account, limit);
            let _ = tx.send((account, generation, result));
        });
    }

    fn invalidate_copy(&mut self, key: TrackKey) {
        self.next_epoch = self.next_epoch.wrapping_add(1);
        self.epochs.insert(key, self.next_epoch);
        if let Some(file) = self.files.get_mut(&key) {
            file.copying = false;
        }
    }

    fn queue_cache_delete(&mut self, key: TrackKey) {
        if let Some(file) = self.files.get(&key) {
            self.cache_deletes
                .push((file.file_id, Instant::now() + Duration::from_secs(2)));
        }
    }

    fn queue_current_cache_delete(&mut self) {
        if self.storage.stream_only
            && let Some(key) = self.state.current_key()
        {
            self.queue_cache_delete(key);
        }
    }

    fn request_cache_cleanup(&mut self, td: &TdJson) {
        if self.cache_clear_extra.is_some() {
            self.cache_cleanup_again = true;
            return;
        }
        let extra = td.next_request_id();
        let request = json!({"@type":"optimizeStorage","size":0,"ttl":0,"count":0,
            "immunity_delay":0,"file_types":[{"@type":"fileTypeAudio"},{"@type":"fileTypeDocument"}],
            "chat_ids":[],"exclude_chat_ids":[],"return_deleted_file_statistics":false,
            "chat_limit":0,"@extra":extra});
        match td.send(&request) {
            Ok(()) => {
                self.cache_clear_extra = Some(extra);
                self.storage.busy = true;
                self.publish_storage();
            }
            Err(error) => {
                self.storage.error = Some(format!("Could not request cache cleanup: {error}"));
                self.publish_storage();
            }
        }
    }

    pub fn storage_command(&mut self, td: &TdJson, command: StorageCommand) {
        let Some(account) = self.account else {
            return;
        };
        match command {
            StorageCommand::Refresh => {
                self.storage.error = None;
                self.refresh_storage();
            }
            StorageCommand::LoadMore => {
                self.visible_limit = self.visible_limit.saturating_add(50);
                self.refresh_storage();
            }
            StorageCommand::SetStreamOnly(enabled) => {
                let Some(store) = &self.store else {
                    return;
                };
                if let Err(error) = store.set_stream_only(account, enabled) {
                    self.storage.error = Some(format!("Could not save Stream only: {error}"));
                    self.publish_storage();
                    return;
                }
                self.storage.stream_only = enabled;
                if enabled {
                    let pending = store.pending_downloads(account).unwrap_or_default();
                    let _ = store.clear_pending(account);
                    for key in pending {
                        self.invalidate_copy(key);
                        if let Some(file) = self.files.get(&key)
                            && self.state.current_key() != Some(key)
                        {
                            let _ = td.send(&json!({"@type":"cancelDownloadFile",
                                "file_id":file.file_id,"only_if_pending":false}));
                        }
                    }
                    self.recover_downloads.clear();
                    self.refresh_current_flags();
                    self.publish();
                    if self.state.current_key().is_none()
                        || self.state.download == DownloadStatus::Saved
                        || matches!(
                            self.state.status,
                            PlaybackStatus::Failed | PlaybackStatus::Idle
                        )
                    {
                        self.request_cache_cleanup(td);
                    }
                }
                self.publish_storage();
                self.refresh_storage();
            }
            StorageCommand::DeleteOffline(key) => {
                if key.account_id != account {
                    return;
                }
                self.delete_one(td, key);
                self.refresh_storage();
            }
            StorageCommand::ClearAudio => {
                self.storage.busy = true;
                self.storage.error = None;
                self.publish_storage();
                let _ = media::dispatch("stop", "{}");
                self.state.status = PlaybackStatus::Paused;
                for read in self.reads.drain(..) {
                    let _ = read.response.send(Err("Audio storage was cleared.".into()));
                }
                let pending = self.store.as_ref().map(|s| s.pending_downloads(account));
                let entries = self.store.as_ref().map(|s| s.saved_entries(account));
                let pending = match pending {
                    Some(Ok(keys)) => keys,
                    _ => {
                        self.storage.error = Some("Could not read pending downloads.".into());
                        Vec::new()
                    }
                };
                let entries = match entries {
                    Some(Ok(rows)) => rows,
                    _ => {
                        self.storage.error = Some("Could not read saved downloads.".into());
                        Vec::new()
                    }
                };
                if let Some(store) = &self.store
                    && store.clear_pending(account).is_err()
                {
                    self.storage.error = Some("Could not cancel all pending downloads.".into());
                }
                for key in pending
                    .into_iter()
                    .chain(entries.iter().map(|(key, _)| *key))
                {
                    self.invalidate_copy(key);
                }
                let mut failures = 0usize;
                for (key, _) in entries {
                    if self
                        .store
                        .as_ref()
                        .is_none_or(|s| s.delete_saved(key, &self.media_root).is_err())
                    {
                        failures += 1;
                    }
                }
                if failures > 0 {
                    self.storage.error = Some(format!(
                        "Could not remove {failures} saved file(s). Try again."
                    ));
                }
                let offline_dir = self.media_root.join(account.to_string());
                if let Err(error) = std::fs::remove_dir_all(&offline_dir)
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    self.storage.error =
                        Some(format!("Could not remove all offline files: {error}"));
                }
                for file in self.files.values() {
                    let _ = td.send(&json!({"@type":"cancelDownloadFile","file_id":file.file_id,
                        "only_if_pending":false}));
                }
                self.cache_deletes.clear();
                self.recover_downloads.clear();
                self.refresh_current_flags();
                self.publish();
                self.request_cache_cleanup(td);
                self.storage.busy = false;
                self.publish_storage();
                self.refresh_storage();
            }
        }
    }

    fn delete_one(&mut self, td: &TdJson, key: TrackKey) {
        self.invalidate_copy(key);
        self.reads.retain(|read| {
            if read.key == key {
                let _ = read
                    .response
                    .send(Err("This saved audio was removed.".into()));
                false
            } else {
                true
            }
        });
        if let Some(store) = &self.store {
            let _ = store.mark_download(key, "canceled", None);
            if self.state.current_key() == Some(key) {
                let _ = media::dispatch("stop", "{}");
                self.state.status = PlaybackStatus::Paused;
            }
            match store.delete_saved(key, &self.media_root) {
                Ok(()) => {
                    if self.state.current_key() == Some(key) {
                        self.state.download = DownloadStatus::Remote;
                        self.publish();
                    }
                    self.queue_cache_delete(key);
                }
                Err(error) => {
                    self.storage.error = Some(error);
                    self.publish_storage();
                }
            }
        }
        if let Some(file) = self.files.get(&key) {
            let _ = td.send(&json!({"@type":"cancelDownloadFile","file_id":file.file_id,
                "only_if_pending":false}));
        }
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
        self.last_recorded_key = None;
        self.state.status = PlaybackStatus::Loading;
        self.state.position_ms = position_ms.max(0);
        self.state.artwork_data = None;
        self.state.error = None;
        self.fallback_key = None;
        self.refresh_current_flags();
        let key = track.key;
        self.fallback_attempted.remove(&key);
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
                        self.queue_current_cache_delete();
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
                    self.queue_current_cache_delete();
                    self.state.cursor -= 1;
                    self.prepare(td);
                }
            }
            PlaybackCommand::Next => {
                self.queue_current_cache_delete();
                if self.state.advance(false).is_some() {
                    self.prepare(td);
                } else {
                    let _ = media::dispatch("stop", "{}");
                    self.state.status = PlaybackStatus::Paused;
                    self.publish();
                }
            }
            PlaybackCommand::Select(key) => {
                if self.state.current_key() != Some(key) {
                    self.queue_current_cache_delete();
                }
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
                if self.storage.stream_only {
                    self.state.error =
                        Some("Turn off Stream only in Storage management to save songs.".into());
                    self.publish();
                    return;
                }
                if let Some(key) = self.state.current_key() {
                    if self
                        .store
                        .as_ref()
                        .and_then(|s| s.saved_path(key).ok().flatten())
                        .is_some()
                    {
                        return;
                    }
                    self.invalidate_copy(key);
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "queued", None);
                    }
                    self.state.download = DownloadStatus::Downloading;
                    self.ensure_file(td, key);
                    if let Some(file) = self.files.get(&key) {
                        let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,"priority":24,"offset":0,"limit":0,"synchronous":false}));
                    }
                    self.start_copy_if_requested(key);
                }
            }
            PlaybackCommand::DownloadTrack(key) => {
                if !self.storage.stream_only
                    && self.account == Some(key.account_id)
                    && self
                        .store
                        .as_ref()
                        .and_then(|s| s.saved_path(key).ok().flatten())
                        .is_none()
                {
                    self.invalidate_copy(key);
                    if let Some(store) = &self.store {
                        let _ = store.mark_download(key, "queued", None);
                    }
                    self.ensure_file(td, key);
                    if let Some(file) = self.files.get(&key) {
                        let _ = td.send(&json!({"@type":"downloadFile","file_id":file.file_id,
                            "priority":24,"offset":0,"limit":0,"synchronous":false}));
                    }
                    self.start_copy_if_requested(key);
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
        let extra = td.next_request_id();
        self.resolving.insert(extra, key);
        let _ = td.send(&json!({"@type":"getMessage","chat_id":key.chat_id,"message_id":key.message_id,"@extra":extra}));
    }

    pub fn read_range(
        &mut self,
        td: &TdJson,
        key: TrackKey,
        offset: i64,
        length: usize,
        response: Sender<Result<Vec<u8>, String>>,
    ) {
        if self.account != Some(key.account_id)
            || self.state.current_key() != Some(key)
            || offset < 0
            || length == 0
        {
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
        if self.cache_clear_extra.is_some() && value["@extra"].as_i64() == self.cache_clear_extra {
            self.cache_clear_extra = None;
            if value["@type"] == "error" {
                self.storage.error =
                    Some("Could not clear all Telegram media cache. Try again.".into());
                self.publish_storage();
            }
            if self.cache_cleanup_again {
                self.cache_cleanup_again = false;
                self.request_cache_cleanup(td);
            }
            self.refresh_storage();
            return true;
        }
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
                if let Some(store) = &self.store
                    && store.download_status(key).ok().flatten().is_some()
                {
                    let _ =
                        store.mark_download(key, "failed", Some("Telegram audio is unavailable"));
                }
                if self.state.current_key() == Some(key) {
                    self.state.error = Some(
                        "This Telegram audio is no longer accessible. Try another track.".into(),
                    );
                    self.state.status = PlaybackStatus::Failed;
                    self.refresh_current_flags();
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
        self.start_copy_if_requested(key);
    }

    fn start_copy_if_requested(&mut self, key: TrackKey) {
        let explicit = self
            .store
            .as_ref()
            .and_then(|store| store.download_status(key).ok().flatten())
            .is_some_and(|status| status == "queued" || status == "downloading");
        let Some(state) = self.files.get_mut(&key) else {
            return;
        };
        if explicit
            && !state.complete
            && let Some(store) = &self.store
        {
            let _ = store.mark_download(key, "downloading", None);
        }
        if explicit && state.complete && !state.copying && !state.path.is_empty() {
            state.copying = true;
            let epoch = *self.epochs.entry(key).or_insert_with(|| {
                self.next_epoch = self.next_epoch.wrapping_add(1);
                self.next_epoch
            });
            let source = state.path.clone();
            let expected = state.size;
            let dest = self
                .media_root
                .join(key.account_id.to_string())
                .join(format!("{}-{}-{epoch}.audio", key.chat_id, key.message_id));
            let tx = self.copy_tx.clone();
            self.copy_workers.push(std::thread::spawn(move || {
                let _ = tx.send((key, epoch, copy_complete(&source, &dest, expected)));
            }));
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

    pub fn media_event(&mut self, td: &TdJson, value: &Value) -> bool {
        let mut persist = true;
        let mut history_changed = false;
        match value["type"].as_str().unwrap_or("") {
            "playing" => {
                self.state.status = PlaybackStatus::Playing;
                if let Some(key) = self.state.current_key()
                    && self.account == Some(key.account_id)
                    && value["media_id"].as_str()
                        == Some(
                            format!("{}:{}:{}", key.account_id, key.chat_id, key.message_id)
                                .as_str(),
                        )
                    && self.last_recorded_key != Some(key)
                    && self
                        .store
                        .as_ref()
                        .is_some_and(|store| store.record_play(key).is_ok())
                {
                    self.last_recorded_key = Some(key);
                    history_changed = true;
                }
            }
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
                    return false;
                }
                let Some(data) = value["data"].as_str().filter(|data| data.len() < 700_000) else {
                    return false;
                };
                self.state.artwork_data = Some(data.to_string());
                persist = false;
            }
            "ended" => {
                self.queue_current_cache_delete();
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
                if self.state.download != DownloadStatus::Saved {
                    self.fallback_key = self
                        .state
                        .current_key()
                        .filter(|key| !self.fallback_attempted.contains(key));
                }
            }
            _ => return false,
        }
        if persist {
            self.persist();
        }
        self.publish();
        history_changed
    }

    fn command_without_library(&mut self, td: &TdJson, next: bool) {
        self.queue_current_cache_delete();
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
        let mut active_workers = Vec::new();
        for worker in self.copy_workers.drain(..) {
            if worker.is_finished() {
                let _ = worker.join();
            } else {
                active_workers.push(worker);
            }
        }
        self.copy_workers = active_workers;
        if self.startup_cleanup_pending {
            self.startup_cleanup_pending = false;
            self.request_cache_cleanup(td);
        }
        for key in std::mem::take(&mut self.recover_downloads) {
            self.ensure_file(td, key);
        }
        self.flush_reads(td);
        if let Some(key) = self.fallback_key
            && let Some(file) = self.files.get(&key)
            && file.complete
            && !file.path.is_empty()
        {
            self.fallback_key = None;
            self.fallback_attempted.insert(key);
            if let Some(track) = self.state.current().filter(|track| track.key == key) {
                let payload = json!({"uri":format!("file://{}",file.path),
                    "title":track.title,"artist":track.artist,
                    "duration":track.duration_seconds.unwrap_or(0) * 1000,
                    "media_id":format!("{}:{}:{}",key.account_id,key.chat_id,key.message_id),
                    "position":self.state.position_ms});
                if media::dispatch("play", &payload.to_string()).is_ok() {
                    self.state.status = PlaybackStatus::Loading;
                    self.state.error = None;
                    self.publish();
                }
            }
        }
        while let Ok((key, epoch, result)) = self.copy_rx.try_recv() {
            let expected = self.epochs.get(&key) == Some(&epoch)
                && self
                    .store
                    .as_ref()
                    .and_then(|s| s.download_status(key).ok().flatten())
                    .is_some_and(|status| status == "queued" || status == "downloading")
                && !self.storage.stream_only;
            if !expected {
                if let Ok((path, _)) = result {
                    let _ = std::fs::remove_file(path);
                    self.refresh_storage();
                }
                continue;
            }
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
                        self.refresh_storage();
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
        while let Ok((account, generation, result)) = self.scan_rx.try_recv() {
            if self.account != Some(account) || self.scan_generation != generation {
                continue;
            }
            match result {
                Ok(mut snapshot) => {
                    snapshot.error = self.storage.error.take();
                    snapshot.busy = self.cache_clear_extra.is_some();
                    self.storage = snapshot;
                }
                Err(error) => {
                    self.storage.error = Some(format!("Could not measure storage: {error}"));
                    self.storage.busy = false;
                }
            }
            self.publish_storage();
        }
        let now = Instant::now();
        let current_file = self
            .state
            .current_key()
            .and_then(|key| self.files.get(&key))
            .map(|file| file.file_id);
        let mut pending = Vec::new();
        let mut cache_changed = false;
        for (file_id, due) in self.cache_deletes.drain(..) {
            if due > now
                || Some(file_id) == current_file
                    && !matches!(
                        self.state.status,
                        PlaybackStatus::Paused | PlaybackStatus::Failed | PlaybackStatus::Idle
                    )
            {
                pending.push((file_id, due));
            } else {
                let _ = td.send(&json!({"@type":"cancelDownloadFile","file_id":file_id,
                    "only_if_pending":false}));
                let _ = td.send(&json!({"@type":"deleteFile","file_id":file_id}));
                cache_changed = true;
            }
        }
        self.cache_deletes = pending;
        if cache_changed {
            self.refresh_storage();
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

fn measure_storage(
    db: &Path,
    data_root: &Path,
    cache_root: &Path,
    account: i64,
    limit: usize,
) -> Result<StorageSnapshot, String> {
    let store = PlaybackStore::open(db)?;
    for (key, _) in store.saved_entries(account)? {
        let _ = store.saved_path(key)?;
    }
    let (download_count, download_bytes, downloads, has_more) =
        store.storage_page(account, limit)?;
    let app_bytes = tree_bytes(data_root)?;
    let cache_bytes = tree_bytes(cache_root)?;
    Ok(StorageSnapshot {
        account_id: Some(account),
        app_bytes,
        download_bytes,
        download_count,
        cache_bytes,
        other_bytes: app_bytes
            .saturating_sub(download_bytes)
            .saturating_sub(cache_bytes),
        stream_only: store.stream_only(account)?,
        downloads,
        has_more,
        ..StorageSnapshot::default()
    })
}

fn tree_bytes(root: &Path) -> Result<u64, String> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.to_string()),
    };
    let mut total = 0u64;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = match std::fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
        };
        if metadata.is_dir() {
            total = total.saturating_add(tree_bytes(&entry.path())?);
        } else if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
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
    let copied = match std::fs::copy(source, &part) {
        Ok(copied) => copied,
        Err(error) => {
            let _ = std::fs::remove_file(&part);
            return Err(storage_error(error));
        }
    };
    if copied != meta.len() {
        let _ = std::fs::remove_file(&part);
        return Err("Incomplete local copy".into());
    }
    if let Err(error) = std::fs::File::open(&part).and_then(|file| file.sync_all()) {
        let _ = std::fs::remove_file(&part);
        return Err(storage_error(error));
    }
    if let Err(error) = std::fs::rename(&part, dest) {
        let _ = std::fs::remove_file(&part);
        return Err(storage_error(error));
    }
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
    use crate::adapters::sqlite::SourceStore;
    use music_core::domain::LibrarySnapshot;

    #[test]
    fn logout_reports_file_cleanup_failure_and_can_retry() {
        let root = std::env::temp_dir().join(format!("tunestash-logout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let files = root.join("tdlib/files");
        std::fs::create_dir_all(&files).unwrap();
        let blocked = root.join("tdlib/offline/7");
        std::fs::create_dir_all(blocked.parent().unwrap()).unwrap();
        std::fs::write(&blocked, b"blocked").unwrap();
        let (playback_tx, _) = watch::channel(PlaybackSnapshot::default());
        let (storage_tx, _) = watch::channel(StorageSnapshot::default());
        let mut engine = PlaybackEngine::new(
            root.join("sources.sqlite").to_str().unwrap(),
            files.to_str().unwrap(),
            playback_tx,
            storage_tx,
        );
        assert!(engine.finish_logout(7).is_err());
        std::fs::remove_file(&blocked).unwrap();
        assert!(engine.finish_logout(7).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

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

    #[test]
    fn streaming_does_not_pin_audio_and_stream_only_blocks_new_saves() {
        let root = std::env::temp_dir().join(format!(
            "tunestash-storage-flow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let files = root.join("files/tdlib/files");
        std::fs::create_dir_all(&files).unwrap();
        let db = root.join("files/sources.sqlite");
        SourceStore::open(&db).unwrap();
        let source = files.join("audio.bin");
        std::fs::write(&source, b"audio bytes").unwrap();
        let (playback_tx, _) = watch::channel(PlaybackSnapshot::default());
        let (storage_tx, _) = watch::channel(StorageSnapshot::default());
        let mut engine = PlaybackEngine::new(
            db.to_str().unwrap(),
            files.to_str().unwrap(),
            playback_tx,
            storage_tx,
        );
        let key = TrackKey {
            account_id: 7,
            chat_id: 8,
            message_id: 9,
        };
        engine.set_account(Some(7));
        let file = json!({"id":42,"size":11,"local":{"path":source,"download_offset":0,
            "downloaded_prefix_size":11,"is_downloading_completed":true}});
        engine.update_file(key, &file);
        engine.tick(&TdJson);
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_none()
        );
        let (library_tx, _) = watch::channel(LibrarySnapshot::default());
        let (search_tx, _) = watch::channel(music_core::domain::SearchSnapshot::default());
        let library = LibraryEngine::new(db.to_str().unwrap(), library_tx, search_tx);
        engine.command(&TdJson, &library, PlaybackCommand::DownloadTrack(key));
        for _ in 0..100 {
            engine.tick(&TdJson);
            if engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_some()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_some()
        );
        engine.storage_command(&TdJson, StorageCommand::SetStreamOnly(true));
        let another = TrackKey {
            message_id: 10,
            ..key
        };
        engine.command(&TdJson, &library, PlaybackCommand::DownloadTrack(another));
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .download_status(another)
                .unwrap()
                .is_none()
        );
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_some()
        );
        drop(engine);
        let (playback_tx, _) = watch::channel(PlaybackSnapshot::default());
        let (storage_tx, _) = watch::channel(StorageSnapshot::default());
        let mut restored = PlaybackEngine::new(
            db.to_str().unwrap(),
            files.to_str().unwrap(),
            playback_tx,
            storage_tx,
        );
        restored.set_account(Some(7));
        assert!(restored.storage.stream_only);
        restored.state.load_queue(
            vec![QueueTrack {
                key,
                title: "Song".into(),
                artist: "Artist".into(),
                filename: "song.mp3".into(),
                duration_seconds: None,
                cover_data: None,
            }],
            key,
        );
        restored.state.status = PlaybackStatus::Playing;
        restored.storage_command(&TdJson, StorageCommand::DeleteOffline(key));
        assert_eq!(restored.state.status, PlaybackStatus::Paused);
        assert_eq!(restored.state.current_key(), Some(key));
        assert!(
            restored
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn canceled_copy_result_cannot_restore_a_deleted_download() {
        let root = std::env::temp_dir().join(format!(
            "tunestash-stale-copy-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("files/tdlib/files")).unwrap();
        let db = root.join("files/sources.sqlite");
        SourceStore::open(&db).unwrap();
        let (playback_tx, _) = watch::channel(PlaybackSnapshot::default());
        let (storage_tx, _) = watch::channel(StorageSnapshot::default());
        let mut engine = PlaybackEngine::new(
            db.to_str().unwrap(),
            root.join("files/tdlib/files").to_str().unwrap(),
            playback_tx,
            storage_tx,
        );
        let key = TrackKey {
            account_id: 1,
            chat_id: 2,
            message_id: 3,
        };
        engine.set_account(Some(1));
        engine.invalidate_copy(key);
        let stale = engine.epochs[&key];
        engine
            .store
            .as_ref()
            .unwrap()
            .mark_download(key, "queued", None)
            .unwrap();
        engine.invalidate_copy(key);
        let copied = root.join("files/tdlib/offline/1/stale.audio");
        std::fs::create_dir_all(copied.parent().unwrap()).unwrap();
        std::fs::write(&copied, b"stale").unwrap();
        engine
            .copy_tx
            .send((key, stale, Ok((copied.clone(), 5))))
            .unwrap();
        engine.tick(&TdJson);
        assert!(!copied.exists());
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn clear_audio_removes_downloads_and_pending_work_without_resetting_setting() {
        let root = std::env::temp_dir().join(format!(
            "tunestash-clear-audio-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let files = root.join("files/tdlib/files");
        let offline = root.join("files/tdlib/offline/1");
        std::fs::create_dir_all(&files).unwrap();
        std::fs::create_dir_all(&offline).unwrap();
        let db = root.join("files/sources.sqlite");
        SourceStore::open(&db).unwrap();
        let key = TrackKey {
            account_id: 1,
            chat_id: 2,
            message_id: 3,
        };
        let pending = TrackKey {
            message_id: 4,
            ..key
        };
        let saved = offline.join("3.audio");
        std::fs::write(&saved, b"audio").unwrap();
        let (playback_tx, _) = watch::channel(PlaybackSnapshot::default());
        let (storage_tx, _) = watch::channel(StorageSnapshot::default());
        let mut engine = PlaybackEngine::new(
            db.to_str().unwrap(),
            files.to_str().unwrap(),
            playback_tx,
            storage_tx,
        );
        engine.set_account(Some(1));
        let store = engine.store.as_ref().unwrap();
        store.save_file(key, &saved, 5).unwrap();
        store.mark_download(pending, "queued", None).unwrap();
        store.set_stream_only(1, true).unwrap();
        engine.storage_command(&TdJson, StorageCommand::ClearAudio);
        assert!(!saved.exists());
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .saved_path(key)
                .unwrap()
                .is_none()
        );
        assert!(
            engine
                .store
                .as_ref()
                .unwrap()
                .pending_downloads(1)
                .unwrap()
                .is_empty()
        );
        assert!(engine.store.as_ref().unwrap().stream_only(1).unwrap());
        let _ = std::fs::remove_dir_all(root);
    }
}

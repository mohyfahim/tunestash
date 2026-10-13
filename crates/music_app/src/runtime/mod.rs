//! Service composition, commands, events, cancellation, and lifecycle ownership.
//!
//! The runtime owns long-running work so unmounting a page does not cancel
//! playback or downloads.

#[cfg(target_os = "android")]
use music_core::domain::TrackKey;
use music_core::{
    domain::{
        AuthCommand, AuthSnapshot, LibraryCommand, LibrarySnapshot, PlaybackCommand,
        PlaybackSnapshot, SourceCommand, SourceSnapshot,
    },
    ports::TelegramAuth,
};
#[cfg(target_os = "android")]
use serde_json::Value;
#[cfg(target_os = "android")]
use std::sync::mpsc::sync_channel;
#[cfg(target_os = "android")]
use std::sync::mpsc::{Sender, channel};
use std::sync::{OnceLock, mpsc::SyncSender};
#[cfg(target_os = "android")]
use std::time::Duration;
use tokio::sync::watch;

static AUTH: OnceLock<Result<AuthService, String>> = OnceLock::new();

pub enum DriverCommand {
    Auth(AuthCommand),
    Source(SourceCommand),
    Library(LibraryCommand),
    Playback(PlaybackCommand),
    #[cfg(target_os = "android")]
    ReadRange {
        key: TrackKey,
        offset: i64,
        length: usize,
        response: Sender<Result<Vec<u8>, String>>,
    },
    #[cfg(target_os = "android")]
    MediaEvent(Value),
}

pub struct AuthService {
    commands: SyncSender<DriverCommand>,
    snapshots: watch::Receiver<AuthSnapshot>,
    sources: watch::Receiver<SourceSnapshot>,
    library: watch::Receiver<LibrarySnapshot>,
    playback: watch::Receiver<PlaybackSnapshot>,
}

impl AuthService {
    pub fn global() -> Result<&'static Self, String> {
        AUTH.get_or_init(Self::start).as_ref().map_err(Clone::clone)
    }

    #[cfg(target_os = "android")]
    fn start() -> Result<Self, String> {
        use crate::{
            adapters::secure_store::android_storage,
            adapters::telegram::{self, TelegramConfig},
        };
        let api_id = option_env!("TELEGRAM_API_ID")
            .ok_or("Telegram credentials are missing from this build.")?
            .parse::<i32>()
            .map_err(|_| "Telegram credentials are invalid in this build.")?;
        let api_hash = option_env!("TELEGRAM_API_HASH")
            .filter(|value| !value.is_empty() && !value.contains("your_"))
            .ok_or("Telegram credentials are missing from this build.")?
            .to_string();
        let storage = android_storage()?;
        let database_dir = format!("{}/tdlib/database", storage.files_dir);
        let files_dir = format!("{}/tdlib/files", storage.files_dir);
        std::fs::create_dir_all(&database_dir)
            .map_err(|_| "Cannot create private Telegram storage.")?;
        std::fs::create_dir_all(&files_dir)
            .map_err(|_| "Cannot create private Telegram storage.")?;
        let config = TelegramConfig {
            api_id,
            api_hash,
            database_dir,
            files_dir,
            database_key: storage.database_key,
            source_db: format!("{}/sources.sqlite", storage.files_dir),
        };
        let (command_tx, command_rx) = sync_channel(16);
        let (snapshot_tx, snapshot_rx) = watch::channel(AuthSnapshot::default());
        let (source_tx, source_rx) = watch::channel(SourceSnapshot::default());
        let (library_tx, library_rx) = watch::channel(LibrarySnapshot::default());
        let (playback_tx, playback_rx) = watch::channel(PlaybackSnapshot::default());
        std::thread::Builder::new()
            .name("tunestash-tdlib".into())
            .spawn(move || {
                telegram::run(
                    config,
                    command_rx,
                    snapshot_tx,
                    source_tx,
                    library_tx,
                    playback_tx,
                )
            })
            .map_err(|_| "Cannot start Telegram connection.".to_string())?;
        Ok(Self {
            commands: command_tx,
            snapshots: snapshot_rx,
            sources: source_rx,
            library: library_rx,
            playback: playback_rx,
        })
    }

    #[cfg(not(target_os = "android"))]
    fn start() -> Result<Self, String> {
        Err("Telegram sign-in is available in the Android app.".into())
    }

    pub fn subscribe(&self) -> watch::Receiver<AuthSnapshot> {
        self.snapshots.clone()
    }

    pub fn source_snapshot(&self) -> SourceSnapshot {
        self.sources.borrow().clone()
    }

    pub fn subscribe_sources(&self) -> watch::Receiver<SourceSnapshot> {
        self.sources.clone()
    }

    pub fn library_snapshot(&self) -> LibrarySnapshot {
        self.library.borrow().clone()
    }

    pub fn subscribe_library(&self) -> watch::Receiver<LibrarySnapshot> {
        self.library.clone()
    }

    pub fn submit_library(&self, command: LibraryCommand) -> Result<(), String> {
        self.commands
            .try_send(DriverCommand::Library(command))
            .map_err(|_| "Telegram is busy. Wait a moment and retry.".into())
    }

    pub fn playback_snapshot(&self) -> PlaybackSnapshot {
        self.playback.borrow().clone()
    }

    pub fn subscribe_playback(&self) -> watch::Receiver<PlaybackSnapshot> {
        self.playback.clone()
    }

    pub fn submit_playback(&self, command: PlaybackCommand) -> Result<(), String> {
        self.commands
            .try_send(DriverCommand::Playback(command))
            .map_err(|_| "Playback is busy. Try again.".into())
    }

    #[cfg(target_os = "android")]
    pub fn read_range(&self, key: TrackKey, offset: i64, length: usize) -> Result<Vec<u8>, String> {
        let (response, result) = channel();
        self.commands
            .send(DriverCommand::ReadRange {
                key,
                offset,
                length,
                response,
            })
            .map_err(|_| "Audio service stopped".to_string())?;
        result
            .recv_timeout(Duration::from_secs(28))
            .map_err(|_| "Audio buffering timed out".to_string())?
    }

    #[cfg(target_os = "android")]
    pub fn submit_media_event(&self, value: Value) -> Result<(), String> {
        self.commands
            .try_send(DriverCommand::MediaEvent(value))
            .map_err(|_| "Playback is busy".into())
    }

    pub fn submit_source(&self, command: SourceCommand) -> Result<(), String> {
        self.commands
            .try_send(DriverCommand::Source(command))
            .map_err(|_| "Telegram is busy. Wait a moment and retry.".into())
    }
}

impl TelegramAuth for AuthService {
    fn snapshot(&self) -> AuthSnapshot {
        self.snapshots.borrow().clone()
    }

    fn submit(&self, command: AuthCommand) -> Result<(), String> {
        self.commands
            .try_send(DriverCommand::Auth(command))
            .map_err(|_| "Telegram is busy. Wait a moment and retry.".into())
    }
}

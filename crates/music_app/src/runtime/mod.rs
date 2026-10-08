//! Service composition, commands, events, cancellation, and lifecycle ownership.
//!
//! The runtime owns long-running work so unmounting a page does not cancel
//! playback or downloads.

use music_core::{
    domain::{AuthCommand, AuthSnapshot},
    ports::TelegramAuth,
};
#[cfg(target_os = "android")]
use std::sync::mpsc::sync_channel;
use std::sync::{OnceLock, mpsc::SyncSender};
use tokio::sync::watch;

static AUTH: OnceLock<Result<AuthService, String>> = OnceLock::new();

pub struct AuthService {
    commands: SyncSender<AuthCommand>,
    snapshots: watch::Receiver<AuthSnapshot>,
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
        };
        let (command_tx, command_rx) = sync_channel(16);
        let (snapshot_tx, snapshot_rx) = watch::channel(AuthSnapshot::default());
        std::thread::Builder::new()
            .name("tunestash-tdlib".into())
            .spawn(move || telegram::run(config, command_rx, snapshot_tx))
            .map_err(|_| "Cannot start Telegram connection.".to_string())?;
        Ok(Self {
            commands: command_tx,
            snapshots: snapshot_rx,
        })
    }

    #[cfg(not(target_os = "android"))]
    fn start() -> Result<Self, String> {
        Err("Telegram sign-in is available in the Android app.".into())
    }

    pub fn subscribe(&self) -> watch::Receiver<AuthSnapshot> {
        self.snapshots.clone()
    }
}

impl TelegramAuth for AuthService {
    fn snapshot(&self) -> AuthSnapshot {
        self.snapshots.borrow().clone()
    }

    fn submit(&self, command: AuthCommand) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|_| "Telegram is busy. Wait a moment and retry.".into())
    }
}

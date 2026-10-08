//! Contracts for the repository, Telegram, media playback, and secure storage.
//!
//! Adapters in `music_app` implement these contracts. The contracts stay in the
//! core so product behavior does not depend on a platform.

use crate::domain::{AuthCommand, AuthSnapshot};

/// Authentication operations exposed to the application runtime.
pub trait TelegramAuth: Send + Sync {
    fn snapshot(&self) -> AuthSnapshot;
    fn submit(&self, command: AuthCommand) -> Result<(), String>;
}

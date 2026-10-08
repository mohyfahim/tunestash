//! Identifiers, tracks, sources, playlists, queue, and availability.
//!
//! Domain types stay free of UI frameworks and platform SDK types.

/// The next input requested by Telegram. UI navigation follows this state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthStage {
    Connecting,
    Phone,
    Code {
        phone: String,
        delivery: String,
        resend_after: Option<u32>,
    },
    EmailAddress,
    EmailCode {
        address_pattern: String,
    },
    Password {
        hint: String,
    },
    Ready {
        user_id: i64,
    },
    Unsupported {
        description: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSnapshot {
    pub stage: AuthStage,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for AuthSnapshot {
    fn default() -> Self {
        Self {
            stage: AuthStage::Connecting,
            busy: false,
            error: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthCommand {
    Retry,
    Phone(String),
    Code(String),
    ResendCode,
    EmailAddress(String),
    EmailCode(String),
    Password(String),
}

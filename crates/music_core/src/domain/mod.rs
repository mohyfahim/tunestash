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

/// Telegram chat identity is scoped to the connected account. IDs retain
/// TDLib's full integer precision and never pass through JavaScript numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    SavedMessages,
    PersonalChannel,
    MusicBot,
    OtherChat,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceChat {
    pub chat_id: i64,
    pub title: String,
    pub subtitle: String,
    pub kind: SourceKind,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryStage {
    Waiting,
    LoadingChats,
    CheckingBots,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub account_id: Option<i64>,
    pub account_name: String,
    pub stage: DiscoveryStage,
    pub checked_bots: usize,
    pub total_bots: usize,
    pub failed_checks: usize,
    pub chats: Vec<SourceChat>,
    pub error: Option<String>,
    pub show_partial: bool,
    pub signing_out: bool,
}

impl Default for SourceSnapshot {
    fn default() -> Self {
        Self {
            account_id: None,
            account_name: String::new(),
            stage: DiscoveryStage::Waiting,
            checked_bots: 0,
            total_bots: 0,
            failed_checks: 0,
            chats: Vec::new(),
            error: None,
            show_partial: false,
            signing_out: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceCommand {
    SetSelected { chat_id: i64, selected: bool },
    RetryDiscovery,
    ContinuePartial,
    LoadMore,
    ChangeAccount,
}

impl SourceSnapshot {
    pub fn default_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats
            .iter()
            .filter(|chat| chat.kind != SourceKind::OtherChat)
    }

    pub fn selected_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats
            .iter()
            .filter(|chat| chat.selected && chat.kind == SourceKind::OtherChat)
    }

    pub fn available_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats
            .iter()
            .filter(|chat| !chat.selected && chat.kind != SourceKind::SavedMessages)
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn selecting_moves_regular_chat_between_sections() {
        let mut snapshot = SourceSnapshot::default();
        snapshot.chats.push(SourceChat {
            chat_id: 42,
            title: "Music group".into(),
            subtitle: String::new(),
            kind: SourceKind::OtherChat,
            selected: false,
        });
        assert_eq!(snapshot.available_chats().count(), 1);
        assert_eq!(snapshot.selected_chats().count(), 0);
        snapshot.chats[0].selected = true;
        assert_eq!(snapshot.available_chats().count(), 0);
        assert_eq!(snapshot.selected_chats().count(), 1);
        snapshot.chats[0].selected = false;
        assert_eq!(snapshot.available_chats().count(), 1);
    }

    #[test]
    fn selected_default_chat_stays_in_defaults_only() {
        let mut snapshot = SourceSnapshot::default();
        snapshot.chats.push(SourceChat {
            chat_id: 7,
            title: "My channel".into(),
            subtitle: String::new(),
            kind: SourceKind::PersonalChannel,
            selected: true,
        });
        assert_eq!(snapshot.default_chats().count(), 1);
        assert_eq!(snapshot.selected_chats().count(), 0);
        assert_eq!(snapshot.available_chats().count(), 0);
    }
}

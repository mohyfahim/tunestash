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
    pub music: MusicCheck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicCheck {
    Unchecked,
    Checking,
    Found,
    Empty,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedChatCheck {
    pub chat_id: i64,
    pub operation: String,
    pub code: i64,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryStage {
    Waiting,
    LoadingChats,
    CheckingMusic,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub account_id: Option<i64>,
    pub account_name: String,
    pub stage: DiscoveryStage,
    pub checked_chats: usize,
    pub total_chats: usize,
    pub failed_checks: usize,
    pub failed_chats: Vec<FailedChatCheck>,
    pub chats: Vec<SourceChat>,
    pub error: Option<String>,
    pub show_partial: bool,
    pub signing_out: bool,
    pub setup_complete: bool,
}

impl Default for SourceSnapshot {
    fn default() -> Self {
        Self {
            account_id: None,
            account_name: String::new(),
            stage: DiscoveryStage::Waiting,
            checked_chats: 0,
            total_chats: 0,
            failed_checks: 0,
            failed_chats: Vec::new(),
            chats: Vec::new(),
            error: None,
            show_partial: false,
            signing_out: false,
            setup_complete: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceCommand {
    SetSelected { chat_id: i64, selected: bool },
    Resync,
    FinishSetup,
    RetryDiscovery,
    ContinuePartial,
    LoadMore,
    ChangeAccount,
}

/// A single Telegram message in the local, metadata-only library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackSummary {
    pub chat_id: i64,
    pub message_id: i64,
    pub title: String,
    pub artist: String,
    pub filename: String,
    pub source_name: String,
    pub date: i64,
    pub duration_seconds: Option<i64>,
    /// TDLib supplies this small JPEG inside some audio message metadata.
    pub cover_data: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibrarySnapshot {
    pub account_id: Option<i64>,
    pub tracks: Vec<TrackSummary>,
    pub total_count: usize,
    pub active_initial: Option<char>,
    pub selected_sources: usize,
    pub indexing_sources: usize,
    pub has_more: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryCommand {
    LoadMore,
    SelectInitial(char),
    Reindex,
    Retry,
}

impl SourceSnapshot {
    pub fn default_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats
            .iter()
            .filter(|chat| chat.music == MusicCheck::Found && chat.kind != SourceKind::OtherChat)
    }

    pub fn selected_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats.iter().filter(|chat| {
            chat.music == MusicCheck::Found && chat.selected && chat.kind == SourceKind::OtherChat
        })
    }

    pub fn available_chats(&self) -> impl Iterator<Item = &SourceChat> {
        self.chats.iter().filter(|chat| {
            chat.music == MusicCheck::Found
                && !chat.selected
                && chat.kind != SourceKind::SavedMessages
        })
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
            music: MusicCheck::Found,
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
            music: MusicCheck::Found,
        });
        assert_eq!(snapshot.default_chats().count(), 1);
        assert_eq!(snapshot.selected_chats().count(), 0);
        assert_eq!(snapshot.available_chats().count(), 0);
    }

    #[test]
    fn unverified_and_empty_chats_are_hidden_from_every_section() {
        let mut snapshot = SourceSnapshot::default();
        for (chat_id, music, selected) in [
            (1, MusicCheck::Unchecked, false),
            (2, MusicCheck::Checking, true),
            (3, MusicCheck::Empty, false),
            (4, MusicCheck::Failed, true),
        ] {
            snapshot.chats.push(SourceChat {
                chat_id,
                title: format!("Chat {chat_id}"),
                subtitle: String::new(),
                kind: SourceKind::OtherChat,
                selected,
                music,
            });
        }
        assert_eq!(snapshot.default_chats().count(), 0);
        assert_eq!(snapshot.selected_chats().count(), 0);
        assert_eq!(snapshot.available_chats().count(), 0);
    }
}

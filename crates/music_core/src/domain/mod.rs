//! Identifiers, tracks, sources, playlists, queue, and availability.
//!
//! Domain types stay free of UI frameworks and platform SDK types.

/// The next input requested by Telegram. UI navigation follows this state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthStage {
    Connecting,
    CleanupFailed {
        description: String,
    },
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
    RetryCleanup,
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
    pub username: Option<String>,
    pub profile_photo_data: Option<String>,
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
            username: None,
            profile_photo_data: None,
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
    pub recent_tracks: Vec<TrackSummary>,
    pub recent_plays: Vec<TrackSummary>,
    pub total_count: usize,
    pub active_initial: Option<char>,
    pub sort: LibrarySort,
    pub selected_sources: usize,
    pub indexing_sources: usize,
    pub has_more: bool,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibrarySort {
    #[default]
    Title,
    Recent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryCommand {
    LoadMore,
    SelectInitial(char),
    SetSort(LibrarySort),
    Reindex,
    Retry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchSnapshot {
    pub account_id: Option<i64>,
    pub query: String,
    pub tracks: Vec<TrackSummary>,
    pub total_count: usize,
    pub has_more: bool,
    pub recent_queries: Vec<String>,
    pub recent_tracks: Vec<TrackSummary>,
    pub selected_sources: usize,
    pub indexing_sources: usize,
    pub error: Option<String>,
    pub indexing_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchCommand {
    SetQuery(String),
    SaveQuery(String),
    ClearHistory,
    LoadMore,
    Retry,
}

/// Stable identity for a Telegram message in the local music library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TrackKey {
    pub account_id: i64,
    pub chat_id: i64,
    pub message_id: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    #[default]
    Off,
    Queue,
    Single,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Queue,
            Self::Queue => Self::Single,
            Self::Single => Self::Off,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    #[default]
    Idle,
    Loading,
    Buffering,
    Playing,
    Paused,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DownloadStatus {
    #[default]
    Remote,
    Downloading,
    Saved,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueTrack {
    pub key: TrackKey,
    pub title: String,
    pub artist: String,
    pub filename: String,
    pub duration_seconds: Option<i64>,
    pub cover_data: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlaybackSnapshot {
    pub queue: Vec<QueueTrack>,
    /// Indices into `queue`, preserving the library order while shuffle is active.
    pub order: Vec<usize>,
    pub cursor: usize,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub status: PlaybackStatus,
    pub download: DownloadStatus,
    pub favorite: bool,
    pub artwork_data: Option<String>,
    pub error: Option<String>,
}

impl PlaybackSnapshot {
    pub fn current(&self) -> Option<&QueueTrack> {
        self.order
            .get(self.cursor)
            .and_then(|index| self.queue.get(*index))
    }

    pub fn current_key(&self) -> Option<TrackKey> {
        self.current().map(|track| track.key)
    }

    pub fn load_queue(&mut self, queue: Vec<QueueTrack>, selected: TrackKey) -> bool {
        let Some(index) = queue.iter().position(|track| track.key == selected) else {
            return false;
        };
        self.order = (0..queue.len()).collect();
        self.queue = queue;
        self.cursor = index;
        self.position_ms = 0;
        self.duration_ms = None;
        self.artwork_data = None;
        self.status = PlaybackStatus::Loading;
        self.error = None;
        true
    }

    pub fn toggle_shuffle(&mut self, seed: u64) {
        let Some(current) = self.order.get(self.cursor).copied() else {
            return;
        };
        self.shuffle = !self.shuffle;
        if self.shuffle {
            let mut upcoming = self.order[self.cursor + 1..].to_vec();
            let mut state = seed.max(1);
            for i in (1..upcoming.len()).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                upcoming.swap(i, state as usize % (i + 1));
            }
            self.order.truncate(self.cursor + 1);
            self.order.extend(upcoming);
        } else {
            let mut played = self.order[..self.cursor].to_vec();
            played.sort_unstable();
            let mut remaining: Vec<_> = (0..self.queue.len())
                .filter(|index| *index != current && !played.contains(index))
                .collect();
            self.order = played;
            self.cursor = self.order.len();
            self.order.push(current);
            self.order.append(&mut remaining);
        }
    }

    pub fn advance(&mut self, natural_end: bool) -> Option<TrackKey> {
        if self.order.is_empty() || (natural_end && self.repeat == RepeatMode::Single) {
            return None;
        }
        if self.cursor + 1 < self.order.len() {
            self.cursor += 1;
        } else if self.repeat == RepeatMode::Queue {
            self.cursor = 0;
        } else {
            return None;
        }
        self.position_ms = 0;
        self.duration_ms = None;
        self.artwork_data = None;
        self.current_key()
    }

    pub fn select(&mut self, key: TrackKey) -> bool {
        let Some(cursor) = self
            .order
            .iter()
            .position(|index| self.queue[*index].key == key)
        else {
            return false;
        };
        self.cursor = cursor;
        self.position_ms = 0;
        self.duration_ms = None;
        self.artwork_data = None;
        true
    }

    /// Remove tracks no longer in enabled sources, preserving the order of
    /// surviving tracks. Returns whether the selected track was removed.
    pub fn retain_available(&mut self, available: &std::collections::HashSet<TrackKey>) -> bool {
        let selected = self.current_key();
        let old_len = self.queue.len();
        if self
            .queue
            .iter()
            .all(|track| available.contains(&track.key))
        {
            return false;
        }
        let mut index_map = vec![None; old_len];
        let mut kept = Vec::with_capacity(old_len);
        for (old_index, track) in self.queue.drain(..).enumerate() {
            if available.contains(&track.key) {
                index_map[old_index] = Some(kept.len());
                kept.push(track);
            }
        }
        let old_order = std::mem::take(&mut self.order);
        let old_cursor = self.cursor;
        self.order = old_order
            .iter()
            .filter_map(|index| index_map.get(*index).copied().flatten())
            .collect();
        self.queue = kept;
        if let Some(selected) = selected
            && let Some(cursor) = self
                .order
                .iter()
                .position(|index| self.queue[*index].key == selected)
        {
            self.cursor = cursor;
            return false;
        }
        let next = old_order
            .iter()
            .skip(old_cursor)
            .chain(old_order.iter().take(old_cursor))
            .filter_map(|index| index_map.get(*index).copied().flatten())
            .find_map(|index| self.order.iter().position(|candidate| *candidate == index));
        self.cursor = next.unwrap_or(0);
        self.position_ms = 0;
        self.duration_ms = None;
        self.artwork_data = None;
        selected.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackCommand {
    PlayLibrary { key: TrackKey, sort: LibrarySort },
    Play,
    Pause,
    Previous,
    Next,
    Select(TrackKey),
    Seek(i64),
    ToggleShuffle,
    CycleRepeat,
    ToggleFavorite,
    Download,
    DownloadTrack(TrackKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAudio {
    pub key: TrackKey,
    pub title: String,
    pub artist: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageSnapshot {
    pub account_id: Option<i64>,
    pub app_bytes: u64,
    pub download_bytes: u64,
    pub download_count: u64,
    pub cache_bytes: u64,
    pub other_bytes: u64,
    pub stream_only: bool,
    pub downloads: Vec<StoredAudio>,
    pub has_more: bool,
    pub busy: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageCommand {
    Refresh,
    LoadMore,
    SetStreamOnly(bool),
    DeleteOffline(TrackKey),
    ClearAudio,
}

#[cfg(test)]
mod playback_tests {
    use super::*;

    fn queue() -> (Vec<QueueTrack>, TrackKey) {
        let tracks: Vec<_> = (0..5)
            .map(|message_id| QueueTrack {
                key: TrackKey {
                    account_id: 1,
                    chat_id: 2,
                    message_id,
                },
                title: message_id.to_string(),
                artist: String::new(),
                filename: String::new(),
                duration_seconds: None,
                cover_data: None,
            })
            .collect();
        let selected = tracks[1].key;
        (tracks, selected)
    }

    #[test]
    fn modes_advance_without_losing_library_order() {
        let (tracks, selected) = queue();
        let mut state = PlaybackSnapshot::default();
        assert!(state.load_queue(tracks, selected));
        state.toggle_shuffle(53);
        assert_eq!(state.current_key(), Some(selected));
        assert_eq!(state.order.len(), 5);
        state.toggle_shuffle(53);
        assert_eq!(state.order, vec![0, 1, 2, 3, 4]);
        assert_eq!(state.advance(true).unwrap().message_id, 2);
        state.repeat = RepeatMode::Single;
        assert_eq!(state.advance(true), None);
        state.repeat = RepeatMode::Queue;
        state.cursor = 4;
        assert_eq!(state.advance(true).unwrap().message_id, 0);
    }

    #[test]
    fn source_reconciliation_keeps_shuffle_order_and_stops_removed_current() {
        use std::collections::HashSet;
        let (tracks, selected) = queue();
        let mut state = PlaybackSnapshot::default();
        state.load_queue(tracks, selected);
        state.toggle_shuffle(53);
        let old_order: Vec<_> = state.order.iter().map(|i| state.queue[*i].key).collect();
        let available: HashSet<_> = state
            .queue
            .iter()
            .map(|track| track.key)
            .filter(|key| key.message_id != 3)
            .collect();
        assert!(!state.retain_available(&available));
        assert_eq!(state.current_key(), Some(selected));
        assert_eq!(
            state
                .order
                .iter()
                .map(|i| state.queue[*i].key)
                .collect::<Vec<_>>(),
            old_order
                .into_iter()
                .filter(|key| available.contains(key))
                .collect::<Vec<_>>()
        );
        let available: HashSet<_> = available
            .into_iter()
            .filter(|key| *key != selected)
            .collect();
        assert!(state.retain_available(&available));
        assert_ne!(state.current_key(), Some(selected));
        assert_eq!(state.position_ms, 0);
    }
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

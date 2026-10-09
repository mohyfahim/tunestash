use super::TdJson;
use crate::adapters::sqlite::SourceStore;
use music_core::domain::{
    DiscoveryStage, FailedChatCheck, MusicCheck, SourceChat, SourceCommand, SourceKind,
    SourceSnapshot,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(35);
// Overlap music searches so empty chats do not wait one-by-one.
const MAX_INFLIGHT: usize = 8;
// Audio filter returns only audio, so one hit is enough to qualify a chat.
const AUDIO_SEARCH_LIMIT: i32 = 1;
const DOCUMENT_SEARCH_LIMIT: i32 = 50;

#[derive(Clone)]
enum Job {
    LoadMain,
    LoadArchive,
    ListMain,
    ListArchive,
    GetChat {
        chat_id: i64,
    },
    User {
        chat_id: i64,
        user_id: i64,
    },
    Channel {
        chat_id: i64,
        supergroup_id: i64,
    },
    Audio {
        chat_id: i64,
        before: i64,
        epoch: u64,
        documents: bool,
    },
}

fn flood_wait_secs(message: &str) -> Option<u64> {
    let upper = message.to_ascii_uppercase();
    let marker = if upper.contains("FLOOD_WAIT") {
        "FLOOD_WAIT"
    } else if upper.contains("RETRY AFTER") {
        "RETRY AFTER"
    } else {
        return None;
    };
    let start = upper.find(marker)? + marker.len();
    let digits: String = message[start..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok().filter(|secs| *secs > 0)
}

impl Job {
    fn chat_id(&self) -> Option<i64> {
        match self {
            Self::LoadMain | Self::LoadArchive | Self::ListMain | Self::ListArchive => None,
            Self::GetChat { chat_id }
            | Self::User { chat_id, .. }
            | Self::Channel { chat_id, .. }
            | Self::Audio { chat_id, .. } => Some(*chat_id),
        }
    }
}

#[derive(Clone)]
struct ChatMeta {
    title: String,
    subtitle: String,
    private_user: Option<i64>,
    supergroup: Option<i64>,
}

pub struct SourceEngine {
    store: Option<SourceStore>,
    snapshot: SourceSnapshot,
    updates: watch::Sender<SourceSnapshot>,
    chats: BTreeMap<i64, ChatMeta>,
    main_ids: BTreeSet<i64>,
    archive_ids: BTreeSet<i64>,
    lists_ready: bool,
    jobs: VecDeque<Job>,
    active: BTreeMap<i64, (Job, Instant)>,
    logout_account: Option<i64>,
    scan_epochs: BTreeMap<i64, u64>,
    proof_messages: BTreeMap<i64, i64>,
    // Chats that already reached a finished music check in this scan. Rechecks
    // must not remove them, or the progress numerator moves backwards.
    finished_checks: BTreeSet<i64>,
    probe_pause_until: Option<Instant>,
}

impl SourceEngine {
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }

    pub fn new(path: &str, updates: watch::Sender<SourceSnapshot>) -> Self {
        let store = SourceStore::open(Path::new(path));
        let mut snapshot = SourceSnapshot::default();
        if store.is_err() {
            snapshot.stage = DiscoveryStage::Failed;
            snapshot.error =
                Some("Can't open source storage. Check available device space and retry.".into());
        }
        updates.send_replace(snapshot.clone());
        Self {
            store: store.ok(),
            snapshot,
            updates,
            chats: BTreeMap::new(),
            main_ids: BTreeSet::new(),
            archive_ids: BTreeSet::new(),
            lists_ready: false,
            jobs: VecDeque::new(),
            active: BTreeMap::new(),
            logout_account: None,
            scan_epochs: BTreeMap::new(),
            proof_messages: BTreeMap::new(),
            finished_checks: BTreeSet::new(),
            probe_pause_until: None,
        }
    }

    fn publish(&self) {
        self.updates.send_replace(self.snapshot.clone());
    }

    fn fail(&mut self, message: &str) {
        // A failed manual resync must not strand the user on an empty source
        // list when a successful catalog was already saved.
        if let (Some(store), Some(account)) = (&self.store, self.snapshot.account_id)
            && let Ok(Some(chats)) = store.load_discovery(account)
        {
            self.snapshot.chats = chats;
            self.snapshot.show_partial = true;
            self.refresh_progress();
        }
        self.snapshot.stage = DiscoveryStage::Failed;
        self.snapshot.error = Some(message.into());
        self.jobs.clear();
        self.active.clear();
        self.probe_pause_until = None;
        self.publish();
    }

    pub fn authorized(&mut self, td: &TdJson, user_id: i64, user: &Value) {
        if self.snapshot.account_id == Some(user_id) {
            return;
        }
        self.snapshot = SourceSnapshot::default();
        self.snapshot.account_id = Some(user_id);
        let first = user["first_name"].as_str().unwrap_or("");
        let last = user["last_name"].as_str().unwrap_or("");
        let display = format!("{first} {last}").trim().to_string();
        self.snapshot.account_name = if display.is_empty() {
            format!("Telegram account {user_id}")
        } else {
            display
        };
        self.jobs.clear();
        self.active.clear();
        self.probe_pause_until = None;
        self.finished_checks.clear();
        self.chats.remove(&user_id);
        let saved_messages = SourceChat {
            chat_id: user_id,
            title: "Saved Messages".into(),
            subtitle: "Your personal Telegram archive".into(),
            kind: SourceKind::SavedMessages,
            selected: false,
            music: MusicCheck::Unchecked,
        };
        let (cached, setup_complete) = match self.store.as_ref() {
            Some(store) => (store.load_discovery(user_id), store.setup_complete(user_id)),
            None => {
                self.fail(
                    "Source storage is unavailable. Restart the app after freeing device space.",
                );
                return;
            }
        };
        self.snapshot.setup_complete = match setup_complete {
            Ok(complete) => complete,
            Err(_) => {
                self.fail("Can't read source setup. Check device storage and retry.");
                return;
            }
        };
        match cached {
            Ok(Some(chats)) => {
                self.snapshot.chats = chats;
                self.snapshot.stage = DiscoveryStage::Complete;
                self.snapshot.show_partial = true;
                self.refresh_progress();
                self.publish();
            }
            Ok(None) => {
                self.add_chat(saved_messages);
                self.restart(td);
            }
            Err(_) => {
                self.fail("Can't read saved source discovery. Check device storage and retry.")
            }
        }
    }

    fn add_chat(&mut self, mut chat: SourceChat) {
        if let (Some(store), Some(account)) = (&self.store, self.snapshot.account_id) {
            match store.selected(account, chat.chat_id) {
                Ok(selected) => chat.selected = selected,
                Err(_) => {
                    self.fail("Can't read saved sources. Check device storage and retry.");
                    return;
                }
            }
        }
        if let Some(existing) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|item| item.chat_id == chat.chat_id)
        {
            chat.music = existing.music;
            *existing = chat;
        } else {
            self.snapshot.chats.push(chat);
        }
        self.snapshot.chats.sort_by(|a, b| {
            (
                a.kind != SourceKind::SavedMessages,
                a.title.to_lowercase(),
                a.chat_id,
            )
                .cmp(&(
                    b.kind != SourceKind::SavedMessages,
                    b.title.to_lowercase(),
                    b.chat_id,
                ))
        });
        self.publish();
    }

    fn restart(&mut self, _td: &TdJson) {
        self.jobs.clear();
        self.active.clear();
        self.probe_pause_until = None;
        self.lists_ready = false;
        self.main_ids.clear();
        self.archive_ids.clear();
        self.snapshot.stage = DiscoveryStage::LoadingChats;
        self.snapshot.checked_chats = 0;
        self.snapshot.total_chats = 0;
        self.snapshot.failed_checks = 0;
        self.snapshot.failed_chats.clear();
        self.snapshot.error = None;
        self.snapshot.show_partial = false;
        self.scan_epochs.clear();
        self.proof_messages.clear();
        self.finished_checks.clear();
        self.snapshot
            .chats
            .retain(|chat| chat.kind == SourceKind::SavedMessages);
        for chat in &mut self.snapshot.chats {
            chat.music = MusicCheck::Unchecked;
        }
        self.jobs.push_back(Job::LoadMain);
        self.publish();
    }

    pub fn handle_command(&mut self, td: &TdJson, command: SourceCommand) {
        match command {
            SourceCommand::SetSelected { chat_id, selected } => {
                let Some(account) = self.snapshot.account_id else {
                    return;
                };
                let Some(chat) = self
                    .snapshot
                    .chats
                    .iter()
                    .find(|chat| chat.chat_id == chat_id && chat.music == MusicCheck::Found)
                    .cloned()
                else {
                    return;
                };
                let result = self
                    .store
                    .as_ref()
                    .ok_or("Source storage is unavailable.".to_string())
                    .and_then(|store| store.set_selected(account, &chat, selected));
                match result {
                    Ok(()) => {
                        if let Some(chat) = self
                            .snapshot
                            .chats
                            .iter_mut()
                            .find(|chat| chat.chat_id == chat_id)
                        {
                            chat.selected = selected;
                        }
                        self.snapshot.error = None;
                        self.publish();
                    }
                    Err(_) => {
                        self.fail("Couldn't save this source. Check device storage and try again.")
                    }
                }
            }
            SourceCommand::RetryDiscovery => {
                if self.store.is_none() {
                    self.fail("Source storage is unavailable. Restart the app after freeing device space.");
                } else if self.snapshot.stage == DiscoveryStage::Complete
                    && !self.snapshot.failed_chats.is_empty()
                {
                    self.retry_failed();
                } else {
                    self.restart(td);
                }
            }
            SourceCommand::Resync => {
                if self.snapshot.account_id.is_some()
                    && self.logout_account.is_none()
                    && !matches!(
                        self.snapshot.stage,
                        DiscoveryStage::LoadingChats | DiscoveryStage::CheckingMusic
                    )
                {
                    self.restart(td);
                }
            }
            SourceCommand::FinishSetup => {
                let Some(account) = self.snapshot.account_id else {
                    return;
                };
                if self.snapshot.stage != DiscoveryStage::Complete
                    || self.snapshot.signing_out
                    || self.snapshot.setup_complete
                {
                    return;
                }
                let saved = self
                    .store
                    .as_ref()
                    .is_some_and(|store| store.finish_setup(account).is_ok());
                if saved {
                    self.snapshot.setup_complete = true;
                    self.refresh_progress();
                } else {
                    self.snapshot.error = Some(
                        "Couldn't save setup progress. Check device storage and retry.".into(),
                    );
                }
                self.publish();
            }
            SourceCommand::ContinuePartial => {
                self.snapshot.show_partial = true;
                self.publish();
            }
            SourceCommand::LoadMore => self.drive(td),
            SourceCommand::ChangeAccount => self.change_account(td),
        }
    }

    pub fn change_account(&mut self, td: &TdJson) {
        let Some(account) = self.snapshot.account_id else {
            return;
        };
        if self.logout_account.is_some() {
            return;
        }
        self.logout_account = Some(account);
        self.jobs.clear();
        self.active.clear();
        self.probe_pause_until = None;
        self.snapshot.signing_out = true;
        self.snapshot.error = None;
        self.publish();
        if td.send(&json!({"@type":"logOut", "@extra":-3})).is_err() {
            self.logout_account = None;
            self.snapshot.signing_out = false;
            self.fail("Couldn't sign out of Telegram. Try again.");
        }
    }

    pub fn signed_out(&mut self) {
        if let Some(account) = self.logout_account.take()
            && let Some(store) = &mut self.store
        {
            let _ = store.clear_account(account);
        }
        self.jobs.clear();
        self.active.clear();
        self.probe_pause_until = None;
        self.chats.clear();
        self.main_ids.clear();
        self.archive_ids.clear();
        self.lists_ready = false;
        self.scan_epochs.clear();
        self.proof_messages.clear();
        self.finished_checks.clear();
        self.snapshot = SourceSnapshot::default();
        self.publish();
    }

    fn ingest_chat(&mut self, chat: &Value) {
        let Some(chat_id) = chat["id"].as_i64() else {
            return;
        };
        if Some(chat_id) == self.snapshot.account_id {
            return;
        }
        let kind = chat["type"]["@type"].as_str().unwrap_or("");
        if kind == "chatTypeSecret" {
            return;
        }
        let private_user = (kind == "chatTypePrivate")
            .then(|| chat["type"]["user_id"].as_i64())
            .flatten();
        let supergroup = (kind == "chatTypeSupergroup")
            .then(|| chat["type"]["supergroup_id"].as_i64())
            .flatten();
        let raw_title = chat["title"].as_str().unwrap_or("").trim();
        let title = if raw_title.is_empty() {
            if kind == "chatTypePrivate" {
                "Telegram user"
            } else {
                "Untitled chat"
            }
        } else {
            raw_title
        }
        .to_string();
        let subtitle = match kind {
            "chatTypePrivate" => "Private chat",
            "chatTypeSupergroup" => "Channel or group",
            "chatTypeBasicGroup" => "Group chat",
            _ => "Telegram chat",
        };
        self.chats.insert(
            chat_id,
            ChatMeta {
                title,
                subtitle: subtitle.into(),
                private_user,
                supergroup,
            },
        );
        if !self.lists_ready || !self.is_listed(chat_id) {
            return;
        }
        self.include_cached_chat(chat_id);
    }

    fn is_listed(&self, chat_id: i64) -> bool {
        self.main_ids.contains(&chat_id) || self.archive_ids.contains(&chat_id)
    }

    fn include_cached_chat(&mut self, chat_id: i64) {
        let Some(meta) = self.chats.get(&chat_id).cloned() else {
            self.jobs.push_back(Job::GetChat { chat_id });
            return;
        };
        if !self.is_listed(chat_id) || self.snapshot.account_id == Some(chat_id) {
            return;
        }
        let existing = self
            .snapshot
            .chats
            .iter()
            .find(|item| item.chat_id == chat_id)
            .cloned();
        let is_new = existing.is_none();
        self.add_chat(SourceChat {
            chat_id,
            title: meta.title,
            subtitle: existing
                .as_ref()
                .filter(|item| item.kind != SourceKind::OtherChat)
                .map(|item| item.subtitle.clone())
                .unwrap_or(meta.subtitle),
            kind: existing.map_or(SourceKind::OtherChat, |item| item.kind),
            selected: false,
            music: MusicCheck::Unchecked,
        });
        if is_new && self.snapshot.stage == DiscoveryStage::CheckingMusic {
            if let Some(user_id) = meta.private_user {
                // Bots and people share chatTypePrivate. Classify first so bots
                // can be checked with channels, and regular DMs wait behind them.
                self.enqueue_job(Job::User { chat_id, user_id });
            } else {
                self.queue_probe(chat_id);
            }
            self.refresh_progress();
            self.publish();
        }
    }

    fn probe_priority(&self, chat_id: i64) -> u8 {
        if self.snapshot.account_id == Some(chat_id) {
            return 0;
        }
        if let Some(chat) = self
            .snapshot
            .chats
            .iter()
            .find(|chat| chat.chat_id == chat_id)
        {
            match chat.kind {
                SourceKind::SavedMessages => return 0,
                SourceKind::PersonalChannel | SourceKind::MusicBot => return 1,
                SourceKind::OtherChat => {
                    if self
                        .chats
                        .get(&chat_id)
                        .is_some_and(|meta| meta.private_user.is_some())
                    {
                        return 3;
                    }
                }
            }
        }
        match self.chats.get(&chat_id) {
            Some(meta) if meta.supergroup.is_some() => 1,
            Some(meta) if meta.private_user.is_some() => 3,
            _ => 4,
        }
    }

    fn job_priority(&self, job: &Job) -> u8 {
        match job {
            Job::Audio { chat_id, .. } => self.probe_priority(*chat_id),
            Job::User { chat_id, .. } => {
                let awaiting_probe =
                    self.snapshot.chats.iter().any(|chat| {
                        chat.chat_id == *chat_id && chat.music == MusicCheck::Unchecked
                    });
                // Classify unchecked private chats before ordinary DM/group probes.
                // Labeling after music is found can wait.
                if awaiting_probe { 2 } else { 6 }
            }
            Job::GetChat { .. } => 1,
            Job::Channel { .. } => 6,
            _ => 0,
        }
    }

    fn enqueue_job(&mut self, job: Job) {
        let priority = self.job_priority(&job);
        let index = self
            .jobs
            .iter()
            .position(|existing| self.job_priority(existing) > priority)
            .unwrap_or(self.jobs.len());
        self.jobs.insert(index, job);
    }

    fn queue_classification(&mut self, chat_id: i64) {
        if self.snapshot.account_id == Some(chat_id) {
            return;
        }
        let Some(meta) = self.chats.get(&chat_id).cloned() else {
            return;
        };
        // Private chats are classified before their music probe.
        if meta.private_user.is_some() {
            return;
        }
        if let Some(supergroup_id) = meta.supergroup {
            self.enqueue_job(Job::Channel {
                chat_id,
                supergroup_id,
            });
        }
    }

    pub fn handle_value(&mut self, _td: &TdJson, value: &Value) -> bool {
        // Source discovery runs only for the first scan or an explicit resync.
        // TDLib keeps sending live chat/message updates after that; consuming
        // them here must not start another source check.
        if !matches!(
            self.snapshot.stage,
            DiscoveryStage::LoadingChats | DiscoveryStage::CheckingMusic
        ) && matches!(
            value["@type"].as_str(),
            Some(
                "updateChatPosition"
                    | "updateChatAddedToList"
                    | "updateChatRemovedFromList"
                    | "updateNewChat"
                    | "updateChatTitle"
                    | "updateNewMessage"
                    | "updateMessageContent"
                    | "updateDeleteMessages"
            )
        ) {
            return true;
        }
        if value["@type"] == "updateChatPosition" {
            if let Some(chat_id) = value["chat_id"].as_i64() {
                let present = value["position"]["order"]
                    .as_i64()
                    .is_some_and(|order| order > 0);
                self.update_list_membership(chat_id, &value["position"]["list"], present);
            }
            return true;
        }
        if value["@type"] == "updateChatAddedToList"
            || value["@type"] == "updateChatRemovedFromList"
        {
            if let Some(chat_id) = value["chat_id"].as_i64() {
                self.update_list_membership(
                    chat_id,
                    &value["chat_list"],
                    value["@type"] == "updateChatAddedToList",
                );
            }
            return true;
        }
        if value["@type"] == "updateNewChat" {
            self.ingest_chat(&value["chat"]);
            return true;
        }
        if value["@type"] == "updateChatTitle" {
            if let (Some(id), Some(title)) = (value["chat_id"].as_i64(), value["title"].as_str()) {
                self.set_title(id, title);
            }
            return true;
        }
        if value["@type"] == "updateNewMessage" {
            let message = &value["message"];
            if super::playable_music_message(message)
                && let (Some(chat_id), Some(message_id)) =
                    (message["chat_id"].as_i64(), message["id"].as_i64())
            {
                self.invalidate_probe(chat_id);
                self.mark_found(chat_id, message_id);
            }
            return true;
        }
        if value["@type"] == "updateMessageContent" {
            if let (Some(chat_id), Some(message_id)) =
                (value["chat_id"].as_i64(), value["message_id"].as_i64())
            {
                if super::playable_music_message(&json!({"content":value["new_content"]})) {
                    self.invalidate_probe(chat_id);
                    self.mark_found(chat_id, message_id);
                } else if self.proof_messages.get(&chat_id) == Some(&message_id) {
                    self.rescan_chat(chat_id);
                }
            }
            return true;
        }
        if value["@type"] == "updateDeleteMessages" {
            // from_cache means TDLib dropped a local copy that can be loaded
            // again. That is not a deletion of the proof message.
            if value["from_cache"] != true
                && let Some(chat_id) = value["chat_id"].as_i64()
                && let Some(proof) = self.proof_messages.get(&chat_id)
                && value["message_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(*proof)))
            {
                self.rescan_chat(chat_id);
            }
            return true;
        }
        if value["@extra"] == -3 && self.logout_account.is_some() {
            if value["@type"] == "error" {
                self.logout_account = None;
                self.snapshot.signing_out = false;
                self.snapshot.error = Some("Couldn't sign out of Telegram. Try again.".into());
                self.publish();
            }
            return true;
        }
        let Some(extra) = value["@extra"].as_i64() else {
            return false;
        };
        let Some((job, _)) = self.active.remove(&extra) else {
            return false;
        };
        if let Job::Audio { chat_id, epoch, .. } = &job
            && self.scan_epochs.get(chat_id).copied().unwrap_or(0) != *epoch
        {
            return true;
        }
        if value["@type"] == "error" {
            if value["code"] == 404 && matches!(job, Job::LoadMain | Job::LoadArchive) {
                self.end_chat_list(job);
            } else {
                let code = value["code"].as_i64().unwrap_or(0);
                let reason = value["message"].as_str().unwrap_or("");
                let job_name = match job {
                    Job::LoadMain | Job::LoadArchive | Job::ListMain | Job::ListArchive => {
                        "chat list"
                    }
                    Job::GetChat { .. } => "chat details",
                    Job::User { .. } => "user",
                    Job::Channel { .. } => "channel",
                    Job::Audio { .. } => "audio history",
                };
                if matches!(
                    job,
                    Job::LoadMain | Job::LoadArchive | Job::ListMain | Job::ListArchive
                ) {
                    self.fail("Couldn't finish loading Telegram chats. Retry or continue with the chats found so far.");
                } else if matches!(job, Job::Audio { .. })
                    && (code == 429 || flood_wait_secs(reason).is_some())
                {
                    let wait = flood_wait_secs(reason).unwrap_or(5).min(60);
                    self.probe_pause_until = Some(Instant::now() + Duration::from_secs(wait));
                    self.enqueue_job(job);
                } else if let Some(chat_id) = job.chat_id() {
                    if matches!(job, Job::GetChat { .. })
                        && !self
                            .snapshot
                            .chats
                            .iter()
                            .any(|chat| chat.chat_id == chat_id)
                    {
                        return true;
                    }
                    if matches!(job, Job::User { .. } | Job::Channel { .. }) {
                        // Classification may be incomplete, but the media check can still succeed.
                    } else if code == 400
                        && reason == "Can't access the chat"
                        && self
                            .snapshot
                            .chats
                            .iter()
                            .any(|chat| chat.chat_id == chat_id && !chat.selected)
                    {
                        self.mark_inaccessible(chat_id);
                    } else {
                        self.mark_failed(chat_id, job_name, code, reason);
                    }
                }
            }
            return true;
        }
        match job {
            Job::LoadMain | Job::LoadArchive => {
                self.jobs.push_front(job);
            }
            Job::ListMain | Job::ListArchive => {
                let Some(ids) = value["chat_ids"].as_array() else {
                    self.fail("Telegram returned an invalid chat list. Retry discovery.");
                    return true;
                };
                let target = if matches!(job, Job::ListMain) {
                    &mut self.main_ids
                } else {
                    &mut self.archive_ids
                };
                target.extend(ids.iter().filter_map(Value::as_i64));
                if matches!(job, Job::ListMain) {
                    self.jobs.push_front(Job::ListArchive);
                } else {
                    self.finish_lists();
                }
            }
            Job::GetChat { chat_id } => {
                if value["type"]["@type"] == "chatTypeSecret" {
                    if self
                        .snapshot
                        .chats
                        .iter()
                        .any(|chat| chat.chat_id == chat_id)
                    {
                        self.mark_failed(
                            chat_id,
                            "chat details",
                            0,
                            "Secret chats are not supported",
                        );
                    }
                } else if value["id"].as_i64() != Some(chat_id) {
                    if self
                        .snapshot
                        .chats
                        .iter()
                        .any(|chat| chat.chat_id == chat_id)
                    {
                        self.mark_failed(
                            chat_id,
                            "chat details",
                            0,
                            "Telegram returned unexpected chat details",
                        );
                    }
                } else {
                    self.ingest_chat(value);
                }
            }
            Job::User { chat_id, .. } => {
                let first = value["first_name"].as_str().unwrap_or("");
                let last = value["last_name"].as_str().unwrap_or("");
                let display = format!("{first} {last}").trim().to_string();
                let username = value["usernames"]["active_usernames"][0]
                    .as_str()
                    .or_else(|| value["username"].as_str());
                let name = if display.is_empty() {
                    username
                        .map(|name| format!("@{name}"))
                        .unwrap_or_else(|| "Telegram user".into())
                } else {
                    display
                };
                self.set_title(chat_id, &name);
                if value["type"]["@type"] == "userTypeBot" {
                    self.set_kind(chat_id, SourceKind::MusicBot, "Bot conversation");
                } else {
                    self.set_kind(chat_id, SourceKind::OtherChat, "Private chat");
                }
                if self.snapshot.stage == DiscoveryStage::CheckingMusic
                    && self
                        .snapshot
                        .chats
                        .iter()
                        .any(|chat| chat.chat_id == chat_id && chat.music == MusicCheck::Unchecked)
                {
                    self.queue_probe(chat_id);
                }
            }
            Job::Channel { chat_id, .. } => {
                if value["is_channel"] == true
                    && value["status"]["@type"] == "chatMemberStatusCreator"
                {
                    self.set_kind(chat_id, SourceKind::PersonalChannel, "Your channel");
                } else {
                    self.set_kind(chat_id, SourceKind::OtherChat, "Channel or group");
                }
            }
            Job::Audio {
                chat_id,
                before,
                epoch,
                documents,
            } => self.process_probe(value, chat_id, before, epoch, documents),
        }
        true
    }

    fn update_list_membership(&mut self, chat_id: i64, list: &Value, present: bool) {
        if !self.lists_ready {
            return;
        }
        let was_listed = self.is_listed(chat_id);
        let ids = match list["@type"].as_str() {
            Some("chatListMain") => &mut self.main_ids,
            Some("chatListArchive") => &mut self.archive_ids,
            _ => return,
        };
        if present {
            ids.insert(chat_id);
        } else {
            ids.remove(&chat_id);
        }
        match (was_listed, self.is_listed(chat_id)) {
            (false, true) => self.include_cached_chat(chat_id),
            (true, false) => {
                self.invalidate_probe(chat_id);
                self.proof_messages.remove(&chat_id);
                self.jobs.retain(|job| job.chat_id() != Some(chat_id));
                self.snapshot.chats.retain(|chat| chat.chat_id != chat_id);
                self.snapshot
                    .failed_chats
                    .retain(|failure| failure.chat_id != chat_id);
                self.refresh_progress();
                self.publish();
            }
            _ => (),
        }
    }

    fn end_chat_list(&mut self, job: Job) {
        match job {
            Job::LoadMain => self.jobs.push_front(Job::LoadArchive),
            Job::LoadArchive => self.jobs.push_front(Job::ListMain),
            _ => (),
        }
    }

    fn finish_lists(&mut self) {
        self.lists_ready = true;
        self.snapshot.stage = DiscoveryStage::CheckingMusic;
        if let Some(account) = self.snapshot.account_id {
            self.queue_probe(account);
        }
        let mut ids: Vec<_> = self.main_ids.union(&self.archive_ids).copied().collect();
        ids.sort_by_key(|chat_id| (self.probe_priority(*chat_id), *chat_id));
        for chat_id in ids {
            self.include_cached_chat(chat_id);
        }
        self.refresh_progress();
        self.publish();
    }

    fn set_kind(&mut self, chat_id: i64, kind: SourceKind, subtitle: &str) {
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.kind = kind;
            chat.subtitle = subtitle.into();
            self.save_discovered_selection(chat_id);
            self.publish();
        }
    }

    fn set_title(&mut self, chat_id: i64, title: &str) {
        if title.trim().is_empty() {
            return;
        }
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.title = title.trim().into();
            self.save_discovered_selection(chat_id);
            self.publish();
        }
    }

    fn save_discovered_selection(&self, chat_id: i64) {
        if let (Some(account), Some(store), Some(chat)) = (
            self.snapshot.account_id,
            &self.store,
            self.snapshot
                .chats
                .iter()
                .find(|chat| chat.chat_id == chat_id && chat.selected),
        ) {
            let _ = store.set_selected(account, chat, true);
        }
    }

    fn refresh_progress(&mut self) {
        for chat in &self.snapshot.chats {
            if matches!(
                chat.music,
                MusicCheck::Found | MusicCheck::Empty | MusicCheck::Failed
            ) {
                self.finished_checks.insert(chat.chat_id);
            }
        }
        self.snapshot.total_chats = self.snapshot.chats.len();
        self.snapshot.checked_chats = self
            .snapshot
            .chats
            .iter()
            .filter(|chat| self.finished_checks.contains(&chat.chat_id))
            .count();
        self.snapshot.failed_checks = self.snapshot.failed_chats.len();
        self.snapshot.error = match self.snapshot.failed_checks {
            0 => None,
            1 => Some("1 chat could not be checked. You can retry discovery.".into()),
            count => Some(format!(
                "{count} chats could not be checked. You can retry discovery."
            )),
        };
    }

    fn queue_probe(&mut self, chat_id: i64) {
        let epoch = self.scan_epochs.get(&chat_id).copied().unwrap_or(0);
        self.enqueue_job(Job::Audio {
            chat_id,
            before: 0,
            epoch,
            documents: false,
        });
    }

    fn retry_failed(&mut self) {
        let failed: Vec<_> = self
            .snapshot
            .failed_chats
            .iter()
            .map(|item| item.chat_id)
            .collect();
        self.snapshot.failed_chats.clear();
        for chat_id in failed {
            self.invalidate_probe(chat_id);
            if let Some(chat) = self
                .snapshot
                .chats
                .iter_mut()
                .find(|chat| chat.chat_id == chat_id)
            {
                chat.music = MusicCheck::Unchecked;
            }
            if self.snapshot.account_id != Some(chat_id) && !self.chats.contains_key(&chat_id) {
                self.enqueue_job(Job::GetChat { chat_id });
            }
            self.queue_probe(chat_id);
        }
        self.snapshot.stage = DiscoveryStage::CheckingMusic;
        self.snapshot.show_partial = true;
        self.refresh_progress();
        self.publish();
    }

    fn invalidate_probe(&mut self, chat_id: i64) {
        let epoch = self.scan_epochs.entry(chat_id).or_insert(0);
        *epoch += 1;
    }

    fn rescan_chat(&mut self, chat_id: i64) {
        if !self
            .snapshot
            .chats
            .iter()
            .any(|chat| chat.chat_id == chat_id)
        {
            return;
        }
        self.invalidate_probe(chat_id);
        self.proof_messages.remove(&chat_id);
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.music = MusicCheck::Unchecked;
        }
        self.snapshot
            .failed_chats
            .retain(|failure| failure.chat_id != chat_id);
        self.queue_probe(chat_id);
        self.snapshot.stage = DiscoveryStage::CheckingMusic;
        self.snapshot.show_partial = true;
        self.refresh_progress();
        self.publish();
    }

    fn mark_found(&mut self, chat_id: i64, message_id: i64) {
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.music = MusicCheck::Found;
            self.proof_messages.insert(chat_id, message_id);
            self.snapshot
                .failed_chats
                .retain(|failure| failure.chat_id != chat_id);
            self.snapshot.show_partial = true;
            self.queue_classification(chat_id);
            self.refresh_progress();
            self.publish();
        }
    }

    fn mark_empty(&mut self, chat_id: i64) {
        let Some(chat) = self
            .snapshot
            .chats
            .iter()
            .find(|chat| chat.chat_id == chat_id)
            .cloned()
        else {
            return;
        };
        if chat.selected {
            let saved = match (self.snapshot.account_id, &self.store) {
                (Some(account), Some(store)) => store.set_selected(account, &chat, false).is_ok(),
                _ => false,
            };
            if !saved {
                self.mark_failed(
                    chat_id,
                    "saving source choice",
                    0,
                    "Couldn't save the updated choice",
                );
                return;
            }
        }
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.selected = false;
            chat.music = MusicCheck::Empty;
        }
        self.proof_messages.remove(&chat_id);
        self.snapshot
            .failed_chats
            .retain(|failure| failure.chat_id != chat_id);
        self.refresh_progress();
        self.publish();
    }

    fn mark_failed(&mut self, chat_id: i64, operation: &str, code: i64, reason: &str) {
        self.proof_messages.remove(&chat_id);
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.music = MusicCheck::Failed;
        }
        self.snapshot
            .failed_chats
            .retain(|failure| failure.chat_id != chat_id);
        self.snapshot.failed_chats.push(FailedChatCheck {
            chat_id,
            operation: operation.into(),
            code,
            reason: reason.into(),
        });
        self.refresh_progress();
        self.publish();
    }

    fn mark_inaccessible(&mut self, chat_id: i64) {
        self.proof_messages.remove(&chat_id);
        if let Some(chat) = self
            .snapshot
            .chats
            .iter_mut()
            .find(|chat| chat.chat_id == chat_id)
        {
            chat.music = MusicCheck::Failed;
        }
        self.refresh_progress();
        self.publish();
    }

    fn process_probe(
        &mut self,
        value: &Value,
        chat_id: i64,
        before: i64,
        epoch: u64,
        documents: bool,
    ) {
        match super::music_probe_page(value, before) {
            super::MusicProbePage::Found(message_id) => self.mark_found(chat_id, message_id),
            super::MusicProbePage::Next(before) => {
                self.enqueue_job(Job::Audio {
                    chat_id,
                    before,
                    epoch,
                    documents,
                });
            }
            super::MusicProbePage::Exhausted if !documents => {
                self.enqueue_job(Job::Audio {
                    chat_id,
                    before: 0,
                    epoch,
                    documents: true,
                });
            }
            super::MusicProbePage::Exhausted => self.mark_empty(chat_id),
            super::MusicProbePage::Invalid(reason) => {
                self.mark_failed(chat_id, "music history", 0, reason);
            }
        }
    }

    fn max_inflight(&self) -> usize {
        match self.snapshot.stage {
            DiscoveryStage::CheckingMusic => MAX_INFLIGHT,
            _ => 1,
        }
    }

    fn probes_paused(&self) -> bool {
        self.probe_pause_until
            .is_some_and(|until| Instant::now() < until)
    }

    fn job_is_stale(&self, job: &Job) -> bool {
        match job {
            Job::Audio { chat_id, epoch, .. } => {
                let status = self
                    .snapshot
                    .chats
                    .iter()
                    .find(|chat| chat.chat_id == *chat_id)
                    .map(|chat| chat.music);
                self.scan_epochs.get(chat_id).copied().unwrap_or(0) != *epoch
                    || matches!(
                        status,
                        None | Some(MusicCheck::Found | MusicCheck::Empty | MusicCheck::Failed)
                    )
            }
            Job::GetChat { chat_id } => self.chats.contains_key(chat_id),
            Job::User { chat_id, .. } | Job::Channel { chat_id, .. } => self
                .snapshot
                .chats
                .iter()
                .any(|chat| chat.chat_id == *chat_id && chat.music == MusicCheck::Failed),
            _ => false,
        }
    }

    fn take_next_job(&mut self) -> Option<Job> {
        if let Some(index) = self.jobs.iter().position(|job| self.job_is_stale(job)) {
            self.jobs.remove(index);
            return self.take_next_job();
        }
        let paused = self.probes_paused();
        let index = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| !(paused && matches!(job, Job::Audio { .. })))
            .min_by_key(|(index, job)| (self.job_priority(job), *index))
            .map(|(index, _)| index)?;
        self.jobs.remove(index)
    }

    fn fail_timeout(&mut self, job: Job) {
        if let Some(chat_id) = job.chat_id() {
            if !matches!(job, Job::GetChat { .. })
                || self
                    .snapshot
                    .chats
                    .iter()
                    .any(|chat| chat.chat_id == chat_id)
            {
                self.mark_failed(chat_id, "Telegram request", 0, "Request timed out");
            }
        } else {
            self.fail(
                "Telegram took too long to load chats. Retry or continue with the chats found so far.",
            );
        }
    }

    fn start_job(&mut self, td: &TdJson, job: Job) {
        let request = match &job {
            Job::LoadMain => {
                json!({"@type":"loadChats", "chat_list":{"@type":"chatListMain"}, "limit":100})
            }
            Job::LoadArchive => {
                json!({"@type":"loadChats", "chat_list":{"@type":"chatListArchive"}, "limit":100})
            }
            Job::ListMain => {
                json!({"@type":"getChats", "chat_list":{"@type":"chatListMain"}, "limit":100000})
            }
            Job::ListArchive => {
                json!({"@type":"getChats", "chat_list":{"@type":"chatListArchive"}, "limit":100000})
            }
            Job::GetChat { chat_id } => json!({"@type":"getChat", "chat_id":chat_id}),
            Job::User { user_id, .. } => json!({"@type":"getUser", "user_id":user_id}),
            Job::Channel { supergroup_id, .. } => {
                json!({"@type":"getSupergroup", "supergroup_id":supergroup_id})
            }
            Job::Audio {
                chat_id,
                before,
                documents,
                ..
            } => {
                json!({"@type":"searchChatMessages", "chat_id":chat_id,
                    "topic_id":null, "query":"", "sender_id":null,
                    "from_message_id":before, "offset":0,
                    "limit":if *documents { DOCUMENT_SEARCH_LIMIT } else { AUDIO_SEARCH_LIMIT },
                    "filter":{"@type":if *documents { "searchMessagesFilterDocument" } else { "searchMessagesFilterAudio" }}})
            }
        };
        let extra = td.next_request_id();
        let mut request = request;
        request["@extra"] = json!(extra);
        if td.send(&request).is_err() {
            if let Some(chat_id) = job.chat_id() {
                if !matches!(job, Job::GetChat { .. })
                    || self
                        .snapshot
                        .chats
                        .iter()
                        .any(|chat| chat.chat_id == chat_id)
                {
                    self.mark_failed(chat_id, "Telegram request", 0, "Couldn't send request");
                }
            } else {
                self.fail("Couldn't request Telegram chats. Check your connection and retry.");
            }
        } else {
            if let Job::Audio { chat_id, .. } = job
                && let Some(chat) = self
                    .snapshot
                    .chats
                    .iter_mut()
                    .find(|chat| chat.chat_id == chat_id)
            {
                chat.music = MusicCheck::Checking;
            }
            self.active.insert(extra, (job, Instant::now()));
        }
    }

    fn complete_music_check_if_idle(&mut self) {
        if self.snapshot.stage != DiscoveryStage::CheckingMusic
            || !self.active.is_empty()
            || !self.jobs.is_empty()
        {
            return;
        }
        if self.snapshot.failed_checks == 0 {
            let Some(account) = self.snapshot.account_id else {
                return;
            };
            let saved = self
                .store
                .as_mut()
                .is_some_and(|store| store.save_discovery(account, &self.snapshot.chats).is_ok());
            if !saved {
                self.fail("Couldn't save discovered sources. Check device storage and retry.");
                return;
            }
        }
        self.snapshot.stage = DiscoveryStage::Complete;
        self.snapshot.show_partial = true;
        self.publish();
    }

    pub fn drive(&mut self, td: &TdJson) {
        if self.logout_account.is_some()
            || self.snapshot.account_id.is_none()
            || !matches!(
                self.snapshot.stage,
                DiscoveryStage::LoadingChats | DiscoveryStage::CheckingMusic
            )
        {
            return;
        }
        let timed_out: Vec<_> = self
            .active
            .iter()
            .filter(|(_, (_, started))| started.elapsed() > REQUEST_TIMEOUT)
            .map(|(extra, _)| *extra)
            .collect();
        for extra in timed_out {
            if let Some((job, _)) = self.active.remove(&extra) {
                self.fail_timeout(job);
            }
        }
        if matches!(
            self.snapshot.stage,
            DiscoveryStage::Failed | DiscoveryStage::Complete
        ) {
            return;
        }
        while self.active.len() < self.max_inflight() {
            let Some(job) = self.take_next_job() else {
                break;
            };
            self.start_job(td, job);
            if matches!(
                self.snapshot.stage,
                DiscoveryStage::Failed | DiscoveryStage::Complete
            ) {
                return;
            }
        }
        self.complete_music_check_if_idle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected_engine(label: &str) -> (SourceEngine, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "tunestash-{label}-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("thread")
        ));
        let _ = std::fs::remove_file(&path);
        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut engine = SourceEngine::new(path.to_str().unwrap(), updates);
        engine.snapshot.account_id = Some(7);
        let chat = SourceChat {
            chat_id: 42,
            title: "Music group".into(),
            subtitle: "Group chat".into(),
            kind: SourceKind::OtherChat,
            selected: true,
            music: MusicCheck::Found,
        };
        engine
            .store
            .as_ref()
            .unwrap()
            .set_selected(7, &chat, true)
            .unwrap();
        engine.add_chat(chat);
        (engine, path)
    }

    #[test]
    fn empty_history_disables_persisted_choice() {
        let (mut engine, path) = selected_engine("empty");
        engine.mark_empty(42);
        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Empty);
        assert!(!engine.snapshot.chats[0].selected);
        assert!(!engine.store.as_ref().unwrap().selected(7, 42).unwrap());
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_history_preserves_persisted_choice_but_hides_chat() {
        let (mut engine, path) = selected_engine("failed");
        engine.mark_failed(42, "audio history", 500, "Temporary error");
        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Failed);
        assert!(engine.snapshot.selected_chats().next().is_none());
        assert!(engine.store.as_ref().unwrap().selected(7, 42).unwrap());
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inaccessible_unselected_chat_does_not_raise_retry_failure() {
        let (mut engine, path) = selected_engine("inaccessible");
        engine.snapshot.chats[0].selected = false;
        engine
            .store
            .as_ref()
            .unwrap()
            .set_selected(7, &engine.snapshot.chats[0], false)
            .unwrap();
        engine.active.insert(
            99,
            (
                Job::Audio {
                    chat_id: 42,
                    before: 0,
                    epoch: 0,
                    documents: false,
                },
                Instant::now(),
            ),
        );
        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@extra":99,"@type":"error","code":400,"message":"Can't access the chat"
            })
        ));
        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Failed);
        assert_eq!(engine.snapshot.failed_checks, 0);
        assert!(engine.snapshot.available_chats().next().is_none());
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn only_main_and_archive_ids_become_candidates() {
        let path =
            std::env::temp_dir().join(format!("tunestash-lists-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut engine = SourceEngine::new(path.to_str().unwrap(), updates);
        engine.snapshot.account_id = Some(7);
        for id in [10, 11, 12] {
            engine.ingest_chat(&json!({
                "id":id,"title":format!("Chat {id}"),
                "type":{"@type":"chatTypeBasicGroup"}
            }));
        }
        engine.main_ids.insert(10);
        engine.archive_ids.insert(10);
        engine.archive_ids.insert(11);
        engine.finish_lists();
        let ids: BTreeSet<_> = engine
            .snapshot
            .chats
            .iter()
            .map(|chat| chat.chat_id)
            .collect();
        assert_eq!(ids, BTreeSet::from([10, 11]));
        assert!(!engine.jobs.iter().any(|job| job.chat_id() == Some(12)));
        engine.update_list_membership(10, &json!({"@type":"chatListMain"}), false);
        assert!(engine.snapshot.chats.iter().any(|chat| chat.chat_id == 10));
        engine.update_list_membership(10, &json!({"@type":"chatListArchive"}), false);
        assert!(!engine.snapshot.chats.iter().any(|chat| chat.chat_id == 10));
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn completed_discovery_survives_restart_without_a_new_scan() {
        let (mut engine, path) = selected_engine("restore-discovery");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.refresh_progress();
        engine.drive(&TdJson);
        assert_eq!(engine.snapshot.stage, DiscoveryStage::Complete);
        drop(engine);

        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut restored = SourceEngine::new(path.to_str().unwrap(), updates);
        restored.authorized(&TdJson, 7, &json!({"first_name":"Test"}));
        assert_eq!(restored.snapshot.stage, DiscoveryStage::Complete);
        assert!(restored.jobs.is_empty());
        assert!(restored.snapshot.chats[0].selected);
        assert_eq!(restored.snapshot.chats[0].music, MusicCheck::Found);
        drop(restored);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn live_updates_wait_for_manual_resync_after_completion() {
        let (mut engine, path) = selected_engine("manual-resync");
        engine.snapshot.stage = DiscoveryStage::Complete;
        engine.lists_ready = true;
        engine.main_ids.insert(43);
        engine.handle_value(
            &TdJson,
            &json!({"@type":"updateNewChat","chat":{
                "id":43,"title":"New chat","type":{"@type":"chatTypeBasicGroup"}
            }}),
        );
        engine.handle_value(
            &TdJson,
            &json!({"@type":"updateNewMessage","message":{
                "chat_id":42,"id":100,"content":{"@type":"messageAudio"}
            }}),
        );
        assert_eq!(engine.snapshot.stage, DiscoveryStage::Complete);
        assert_eq!(engine.snapshot.chats.len(), 1);
        assert!(engine.jobs.is_empty());

        engine.handle_command(&TdJson, SourceCommand::Resync);
        assert_eq!(engine.snapshot.stage, DiscoveryStage::LoadingChats);
        assert!(matches!(engine.jobs.front(), Some(Job::LoadMain)));
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_manual_resync_restores_last_completed_catalog() {
        let (mut engine, path) = selected_engine("failed-resync");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.refresh_progress();
        engine.drive(&TdJson);
        engine.handle_command(&TdJson, SourceCommand::Resync);
        assert!(engine.snapshot.selected_chats().next().is_none());

        engine.fail("Telegram is unavailable.");

        assert_eq!(engine.snapshot.stage, DiscoveryStage::Failed);
        assert!(engine.snapshot.show_partial);
        assert_eq!(engine.snapshot.selected_chats().count(), 1);
        assert_eq!(
            engine.snapshot.error.as_deref(),
            Some("Telegram is unavailable.")
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn finish_setup_accepts_complete_scan_with_gaps_and_no_selection() {
        let (mut engine, path) = selected_engine("finish-setup");
        engine.snapshot.chats[0].selected = false;
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.handle_command(&TdJson, SourceCommand::FinishSetup);
        assert!(!engine.snapshot.setup_complete);

        engine.snapshot.stage = DiscoveryStage::Complete;
        engine.snapshot.failed_chats.push(FailedChatCheck {
            chat_id: 99,
            operation: "Checking music".into(),
            code: 500,
            reason: "Telegram timed out".into(),
        });
        engine.refresh_progress();
        assert_eq!(engine.snapshot.failed_checks, 1);
        engine.handle_command(&TdJson, SourceCommand::FinishSetup);
        assert!(engine.snapshot.setup_complete);
        assert_eq!(engine.snapshot.failed_checks, 1);
        assert!(engine.store.as_ref().unwrap().setup_complete(7).unwrap());
        drop(engine);

        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut restored = SourceEngine::new(path.to_str().unwrap(), updates);
        restored.authorized(&TdJson, 7, &json!({"first_name":"Test"}));
        assert!(restored.snapshot.setup_complete);
        drop(restored);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_setup_write_keeps_sources_open_and_reports_error() {
        let (mut engine, path) = selected_engine("finish-setup-error");
        engine.snapshot.stage = DiscoveryStage::Complete;
        engine.store = None;

        engine.handle_command(&TdJson, SourceCommand::FinishSetup);

        assert_eq!(engine.snapshot.stage, DiscoveryStage::Complete);
        assert!(!engine.snapshot.setup_complete);
        assert!(
            engine
                .snapshot
                .error
                .as_deref()
                .unwrap()
                .contains("save setup")
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rescanning_a_finished_chat_does_not_lower_progress() {
        let (mut engine, path) = selected_engine("progress-rescan");
        engine.add_chat(SourceChat {
            chat_id: 43,
            title: "Other group".into(),
            subtitle: "Group chat".into(),
            kind: SourceKind::OtherChat,
            selected: false,
            music: MusicCheck::Unchecked,
        });
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.mark_found(42, 100);
        engine.mark_found(43, 200);
        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (2, 2)
        );

        engine.rescan_chat(42);
        engine.rescan_chat(43);

        assert_eq!(
            engine
                .snapshot
                .chats
                .iter()
                .filter(|chat| chat.music == MusicCheck::Unchecked)
                .count(),
            2
        );
        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (2, 2)
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cache_only_delete_does_not_recheck_a_finished_chat() {
        let (mut engine, path) = selected_engine("progress-cache-delete");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.mark_found(42, 100);
        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (1, 1)
        );

        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@type": "updateDeleteMessages",
                "chat_id": 42,
                "message_ids": [100],
                "from_cache": true
            })
        ));

        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Found);
        assert!(
            engine
                .jobs
                .iter()
                .all(|job| !matches!(job, Job::Audio { .. }))
        );
        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (1, 1)
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn permanent_delete_rechecks_without_lowering_progress() {
        let (mut engine, path) = selected_engine("progress-permanent-delete");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.mark_found(42, 100);

        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@type": "updateDeleteMessages",
                "chat_id": 42,
                "message_ids": [100],
                "from_cache": false
            })
        ));

        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Unchecked);
        assert!(engine.jobs.iter().any(|job| job.chat_id() == Some(42)));
        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (1, 1)
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_new_scan_starts_progress_at_zero() {
        let (mut engine, path) = selected_engine("progress-restart");
        engine.mark_found(42, 100);
        assert_eq!(engine.snapshot.checked_chats, 1);

        engine.restart(&TdJson);
        engine.add_chat(SourceChat {
            chat_id: 42,
            title: "Music group".into(),
            subtitle: "Group chat".into(),
            kind: SourceKind::OtherChat,
            selected: false,
            music: MusicCheck::Unchecked,
        });
        engine.refresh_progress();

        assert_eq!(
            (engine.snapshot.checked_chats, engine.snapshot.total_chats),
            (0, 1)
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn music_checks_run_concurrently() {
        let (mut engine, path) = selected_engine("concurrent-probes");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.snapshot.chats[0].music = MusicCheck::Unchecked;
        for chat_id in 43..(43 + MAX_INFLIGHT as i64) {
            engine.add_chat(SourceChat {
                chat_id,
                title: format!("Chat {chat_id}"),
                subtitle: "Group chat".into(),
                kind: SourceKind::OtherChat,
                selected: false,
                music: MusicCheck::Unchecked,
            });
            engine.queue_probe(chat_id);
        }
        engine.queue_probe(42);
        engine.drive(&TdJson);
        assert_eq!(engine.active.len(), MAX_INFLIGHT);
        assert!(
            engine
                .active
                .values()
                .all(|(job, _)| matches!(job, Job::Audio { .. }))
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn private_chats_classify_before_music_probe() {
        let (mut engine, path) = selected_engine("classify-private");
        engine.lists_ready = true;
        engine.main_ids.insert(50);
        engine.chats.insert(
            50,
            ChatMeta {
                title: "Music bot".into(),
                subtitle: "Private chat".into(),
                private_user: Some(50),
                supergroup: None,
            },
        );
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.include_cached_chat(50);
        assert!(
            engine
                .jobs
                .iter()
                .any(|job| matches!(job, Job::User { chat_id: 50, .. }))
        );
        assert!(
            engine
                .jobs
                .iter()
                .all(|job| !matches!(job, Job::Audio { chat_id: 50, .. }))
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn channels_defer_labeling_until_music_is_found() {
        let (mut engine, path) = selected_engine("defer-channel");
        engine.lists_ready = true;
        engine.main_ids.insert(60);
        engine.chats.insert(
            60,
            ChatMeta {
                title: "Channel".into(),
                subtitle: "Channel or group".into(),
                private_user: None,
                supergroup: Some(60),
            },
        );
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.include_cached_chat(60);
        assert!(
            engine
                .jobs
                .iter()
                .any(|job| matches!(job, Job::Audio { chat_id: 60, .. }))
        );
        assert!(
            engine
                .jobs
                .iter()
                .all(|job| !matches!(job, Job::Channel { .. }))
        );
        engine.mark_found(60, 9);
        assert!(
            engine
                .jobs
                .iter()
                .any(|job| matches!(job, Job::Channel { chat_id: 60, .. }))
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_audio_history_checks_documents_before_marking_chat_empty() {
        let (mut engine, path) = selected_engine("audio-only-empty");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.snapshot.chats[0].music = MusicCheck::Checking;
        engine.chats.insert(
            42,
            ChatMeta {
                title: "Music bot".into(),
                subtitle: "Private chat".into(),
                private_user: Some(42),
                supergroup: None,
            },
        );
        engine.process_probe(
            &json!({"messages":[],"next_from_message_id":0}),
            42,
            0,
            0,
            false,
        );
        assert!(engine.jobs.iter().any(|job| matches!(
            job,
            Job::Audio {
                documents: true,
                ..
            }
        )));
        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Checking);
        engine.jobs.clear();
        engine.process_probe(
            &json!({"messages":[],"next_from_message_id":0}),
            42,
            0,
            0,
            true,
        );
        assert_eq!(engine.snapshot.chats[0].music, MusicCheck::Empty);
        assert!(engine.jobs.is_empty());
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn music_probes_start_before_channel_labeling() {
        let (mut engine, path) = selected_engine("probe-priority");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.snapshot.chats[0].music = MusicCheck::Unchecked;
        engine.chats.insert(
            42,
            ChatMeta {
                title: "Channel".into(),
                subtitle: "Channel or group".into(),
                private_user: None,
                supergroup: Some(42),
            },
        );
        engine.enqueue_job(Job::Channel {
            chat_id: 42,
            supergroup_id: 42,
        });
        for chat_id in 43..(43 + MAX_INFLIGHT as i64) {
            engine.add_chat(SourceChat {
                chat_id,
                title: format!("Chat {chat_id}"),
                subtitle: "Group chat".into(),
                kind: SourceKind::OtherChat,
                selected: false,
                music: MusicCheck::Unchecked,
            });
            engine.queue_probe(chat_id);
        }
        engine.drive(&TdJson);
        assert_eq!(engine.active.len(), MAX_INFLIGHT);
        assert!(
            engine
                .active
                .values()
                .all(|(job, _)| matches!(job, Job::Audio { .. }))
        );
        assert!(
            engine
                .jobs
                .iter()
                .any(|job| matches!(job, Job::Channel { chat_id: 42, .. }))
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn flood_wait_pauses_probes_and_retries_the_request() {
        let (mut engine, path) = selected_engine("flood-wait");
        engine.snapshot.stage = DiscoveryStage::CheckingMusic;
        engine.snapshot.chats[0].music = MusicCheck::Checking;
        engine.active.insert(
            99,
            (
                Job::Audio {
                    chat_id: 42,
                    before: 0,
                    epoch: 0,
                    documents: false,
                },
                Instant::now(),
            ),
        );
        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@extra": 99,
                "@type": "error",
                "code": 429,
                "message": "FLOOD_WAIT_8"
            })
        ));
        assert!(engine.probe_pause_until.is_some());
        assert!(
            engine
                .jobs
                .iter()
                .any(|job| matches!(job, Job::Audio { chat_id: 42, .. }))
        );
        assert!(engine.active.is_empty());
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn finish_lists_orders_saved_channels_then_private_classification() {
        let path =
            std::env::temp_dir().join(format!("tunestash-priority-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut engine = SourceEngine::new(path.to_str().unwrap(), updates);
        engine.snapshot.account_id = Some(7);
        engine.chats.insert(
            20,
            ChatMeta {
                title: "Group".into(),
                subtitle: "Group chat".into(),
                private_user: None,
                supergroup: None,
            },
        );
        engine.chats.insert(
            30,
            ChatMeta {
                title: "Person".into(),
                subtitle: "Private chat".into(),
                private_user: Some(30),
                supergroup: None,
            },
        );
        engine.chats.insert(
            40,
            ChatMeta {
                title: "Channel".into(),
                subtitle: "Channel or group".into(),
                private_user: None,
                supergroup: Some(40),
            },
        );
        engine.main_ids.extend([20, 30, 40]);
        engine.finish_lists();
        let job_kinds: Vec<_> = engine
            .jobs
            .iter()
            .map(|job| match job {
                Job::Audio { chat_id, .. } => format!("audio:{chat_id}"),
                Job::User { chat_id, .. } => format!("user:{chat_id}"),
                other => format!("other:{}", other.chat_id().unwrap_or_default()),
            })
            .collect();
        assert_eq!(
            job_kinds,
            vec![
                "audio:7".to_string(),
                "audio:40".to_string(),
                "user:30".to_string(),
                "audio:20".to_string(),
            ]
        );

        engine.active.insert(
            500,
            (
                Job::User {
                    chat_id: 30,
                    user_id: 30,
                },
                Instant::now(),
            ),
        );
        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@extra": 500,
                "@type": "user",
                "id": 30,
                "first_name": "Music",
                "last_name": "Bot",
                "type": {"@type": "userTypeBot"}
            })
        ));
        let probe_ids: Vec<_> = engine
            .jobs
            .iter()
            .filter_map(|job| match job {
                Job::Audio { chat_id, .. } => Some(*chat_id),
                _ => None,
            })
            .collect();
        // Bot music checks join the channel tier, ahead of ordinary groups.
        assert_eq!(probe_ids, vec![7, 40, 30, 20]);
        assert_eq!(
            engine
                .snapshot
                .chats
                .iter()
                .find(|chat| chat.chat_id == 30)
                .map(|chat| chat.kind),
            Some(SourceKind::MusicBot)
        );
        drop(engine);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn regular_private_chats_wait_behind_channels_and_bots() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-private-priority-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let (updates, _) = watch::channel(SourceSnapshot::default());
        let mut engine = SourceEngine::new(path.to_str().unwrap(), updates);
        engine.snapshot.account_id = Some(7);
        engine.chats.insert(
            20,
            ChatMeta {
                title: "Group".into(),
                subtitle: "Group chat".into(),
                private_user: None,
                supergroup: None,
            },
        );
        engine.chats.insert(
            30,
            ChatMeta {
                title: "Friend".into(),
                subtitle: "Private chat".into(),
                private_user: Some(30),
                supergroup: None,
            },
        );
        engine.chats.insert(
            40,
            ChatMeta {
                title: "Channel".into(),
                subtitle: "Channel or group".into(),
                private_user: None,
                supergroup: Some(40),
            },
        );
        engine.main_ids.extend([20, 30, 40]);
        engine.finish_lists();
        engine.active.insert(
            501,
            (
                Job::User {
                    chat_id: 30,
                    user_id: 30,
                },
                Instant::now(),
            ),
        );
        assert!(engine.handle_value(
            &TdJson,
            &json!({
                "@extra": 501,
                "@type": "user",
                "id": 30,
                "first_name": "Friend",
                "type": {"@type": "userTypeRegular"}
            })
        ));
        let probe_ids: Vec<_> = engine
            .jobs
            .iter()
            .filter_map(|job| match job {
                Job::Audio { chat_id, .. } => Some(*chat_id),
                _ => None,
            })
            .collect();
        assert_eq!(probe_ids, vec![7, 40, 30, 20]);
        drop(engine);
        let _ = std::fs::remove_file(path);
    }
}

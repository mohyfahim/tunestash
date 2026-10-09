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
    },
    Document {
        chat_id: i64,
        before: i64,
        epoch: u64,
    },
}

impl Job {
    fn chat_id(&self) -> Option<i64> {
        match self {
            Self::LoadMain | Self::LoadArchive | Self::ListMain | Self::ListArchive => None,
            Self::GetChat { chat_id }
            | Self::User { chat_id, .. }
            | Self::Channel { chat_id, .. }
            | Self::Audio { chat_id, .. }
            | Self::Document { chat_id, .. } => Some(*chat_id),
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
    active: Option<(i64, Job, Instant)>,
    next_extra: i64,
    logout_account: Option<i64>,
    initial_complete: bool,
    scan_epochs: BTreeMap<i64, u64>,
    proof_messages: BTreeMap<i64, i64>,
}

impl SourceEngine {
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
            active: None,
            next_extra: 1000,
            logout_account: None,
            initial_complete: false,
            scan_epochs: BTreeMap::new(),
            proof_messages: BTreeMap::new(),
        }
    }

    fn publish(&self) {
        self.updates.send_replace(self.snapshot.clone());
    }

    fn fail(&mut self, message: &str) {
        self.snapshot.stage = DiscoveryStage::Failed;
        self.snapshot.error = Some(message.into());
        self.jobs.clear();
        self.active = None;
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
        self.active = None;
        self.chats.remove(&user_id);
        self.add_chat(SourceChat {
            chat_id: user_id,
            title: "Saved Messages".into(),
            subtitle: "Your personal Telegram archive".into(),
            kind: SourceKind::SavedMessages,
            selected: false,
            music: MusicCheck::Unchecked,
        });
        if self.store.is_none() {
            self.fail("Source storage is unavailable. Restart the app after freeing device space.");
            return;
        }
        self.restart(td);
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
        self.active = None;
        self.initial_complete = false;
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
        self.active = None;
        self.snapshot.signing_out = true;
        self.snapshot.error = None;
        self.publish();
        if td.send(&json!({"@type":"logOut", "@extra":999})).is_err() {
            self.logout_account = None;
            self.snapshot.signing_out = false;
            self.fail("Couldn't sign out of Telegram. Try again.");
        }
    }

    pub fn signed_out(&mut self) {
        if let Some(account) = self.logout_account.take()
            && let Some(store) = &self.store
        {
            let _ = store.clear_account(account);
        }
        self.jobs.clear();
        self.active = None;
        self.chats.clear();
        self.main_ids.clear();
        self.archive_ids.clear();
        self.lists_ready = false;
        self.scan_epochs.clear();
        self.proof_messages.clear();
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
        if is_new
            && matches!(
                self.snapshot.stage,
                DiscoveryStage::CheckingMusic | DiscoveryStage::Complete
            )
        {
            if let Some(user_id) = meta.private_user {
                self.jobs.push_back(Job::User { chat_id, user_id });
            }
            if let Some(supergroup_id) = meta.supergroup {
                self.jobs.push_back(Job::Channel {
                    chat_id,
                    supergroup_id,
                });
            }
            self.queue_probe(chat_id);
            if self.initial_complete {
                self.snapshot.show_partial = true;
            }
            self.snapshot.stage = DiscoveryStage::CheckingMusic;
            self.refresh_progress();
            self.publish();
        }
    }

    pub fn handle_value(&mut self, _td: &TdJson, value: &Value) -> bool {
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
            if let Some(chat_id) = value["chat_id"].as_i64()
                && let Some(proof) = self.proof_messages.get(&chat_id)
                && value["message_ids"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_i64() == Some(*proof)))
            {
                self.rescan_chat(chat_id);
            }
            return true;
        }
        if value["@extra"] == 999 && self.logout_account.is_some() {
            if value["@type"] == "error" {
                self.logout_account = None;
                self.snapshot.signing_out = false;
                self.snapshot.error = Some("Couldn't sign out of Telegram. Try again.".into());
                self.publish();
            }
            return true;
        }
        let Some((extra, job, _)) = self.active.clone() else {
            return false;
        };
        if value["@extra"].as_i64() != Some(extra) {
            return false;
        }
        self.active = None;
        if let Job::Audio { chat_id, epoch, .. } | Job::Document { chat_id, epoch, .. } = &job
            && self.scan_epochs.get(chat_id).copied().unwrap_or(0) != *epoch
        {
            return true;
        }
        if value["@type"] == "error" {
            if value["code"] == 404 && matches!(job, Job::LoadMain | Job::LoadArchive) {
                self.end_chat_list(job);
            } else {
                let code = value["code"].as_i64().unwrap_or(0);
                let job_name = match job {
                    Job::LoadMain | Job::LoadArchive | Job::ListMain | Job::ListArchive => {
                        "chat list"
                    }
                    Job::GetChat { .. } => "chat details",
                    Job::User { .. } => "user",
                    Job::Channel { .. } => "channel",
                    Job::Audio { .. } => "audio history",
                    Job::Document { .. } => "document history",
                };
                if matches!(
                    job,
                    Job::LoadMain | Job::LoadArchive | Job::ListMain | Job::ListArchive
                ) {
                    self.fail("Couldn't finish loading Telegram chats. Retry or continue with the chats found so far.");
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
                        && value["message"] == "Can't access the chat"
                        && self
                            .snapshot
                            .chats
                            .iter()
                            .any(|chat| chat.chat_id == chat_id && !chat.selected)
                    {
                        self.mark_inaccessible(chat_id);
                    } else {
                        self.mark_failed(
                            chat_id,
                            job_name,
                            code,
                            value["message"].as_str().unwrap_or(""),
                        );
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
            } => self.process_probe(value, chat_id, before, epoch, false),
            Job::Document {
                chat_id,
                before,
                epoch,
            } => self.process_probe(value, chat_id, before, epoch, true),
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
        let ids: BTreeSet<_> = self.main_ids.union(&self.archive_ids).copied().collect();
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
        self.snapshot.total_chats = self.snapshot.chats.len();
        self.snapshot.checked_chats = self
            .snapshot
            .chats
            .iter()
            .filter(|chat| {
                matches!(
                    chat.music,
                    MusicCheck::Found | MusicCheck::Empty | MusicCheck::Failed
                )
            })
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
        self.jobs.push_back(Job::Audio {
            chat_id,
            before: 0,
            epoch,
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
            if self.snapshot.account_id == Some(chat_id) {
                self.queue_probe(chat_id);
            } else if let Some(meta) = self.chats.get(&chat_id) {
                if let Some(user_id) = meta.private_user {
                    self.jobs.push_back(Job::User { chat_id, user_id });
                }
                if let Some(supergroup_id) = meta.supergroup {
                    self.jobs.push_back(Job::Channel {
                        chat_id,
                        supergroup_id,
                    });
                }
                self.queue_probe(chat_id);
            } else {
                self.jobs.push_back(Job::GetChat { chat_id });
            }
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
                self.jobs.push_back(if documents {
                    Job::Document {
                        chat_id,
                        before,
                        epoch,
                    }
                } else {
                    Job::Audio {
                        chat_id,
                        before,
                        epoch,
                    }
                });
            }
            super::MusicProbePage::Exhausted if documents => self.mark_empty(chat_id),
            super::MusicProbePage::Exhausted => {
                self.jobs.push_back(Job::Document {
                    chat_id,
                    before: 0,
                    epoch,
                });
            }
            super::MusicProbePage::Invalid(reason) => {
                self.mark_failed(chat_id, "music history", 0, reason);
            }
        }
    }

    pub fn drive(&mut self, td: &TdJson) {
        if self.logout_account.is_some() || self.snapshot.account_id.is_none() {
            return;
        }
        if let Some((_, job, started)) = &self.active {
            if started.elapsed() > REQUEST_TIMEOUT {
                let job = job.clone();
                self.active = None;
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
                    self.fail("Telegram took too long to load chats. Retry or continue with the chats found so far.");
                }
            }
            return;
        }
        let job = loop {
            let Some(job) = self.jobs.pop_front() else {
                break None;
            };
            if let Job::Audio { chat_id, epoch, .. } | Job::Document { chat_id, epoch, .. } = &job {
                let status = self
                    .snapshot
                    .chats
                    .iter()
                    .find(|chat| chat.chat_id == *chat_id)
                    .map(|chat| chat.music);
                if self.scan_epochs.get(chat_id).copied().unwrap_or(0) != *epoch
                    || matches!(
                        status,
                        None | Some(MusicCheck::Found | MusicCheck::Empty | MusicCheck::Failed)
                    )
                {
                    continue;
                }
            }
            if let Job::GetChat { chat_id } = &job
                && self.chats.contains_key(chat_id)
            {
                continue;
            }
            if let Job::User { chat_id, .. } | Job::Channel { chat_id, .. } = &job
                && self
                    .snapshot
                    .chats
                    .iter()
                    .any(|chat| chat.chat_id == *chat_id && chat.music == MusicCheck::Failed)
            {
                continue;
            }
            break Some(job);
        };
        let Some(job) = job else {
            if self.snapshot.stage == DiscoveryStage::CheckingMusic {
                self.snapshot.stage = DiscoveryStage::Complete;
                self.initial_complete = true;
                self.snapshot.show_partial = true;
                self.publish();
            }
            return;
        };
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
                chat_id, before, ..
            }
            | Job::Document {
                chat_id, before, ..
            } => {
                let filter = if matches!(job, Job::Audio { .. }) {
                    "searchMessagesFilterAudio"
                } else {
                    "searchMessagesFilterDocument"
                };
                json!({"@type":"searchChatMessages", "chat_id":chat_id,
                    "topic_id":null, "query":"", "sender_id":null,
                    "from_message_id":before, "offset":0, "limit":100,
                    "filter":{"@type":filter}})
            }
        };
        self.next_extra += 1;
        let extra = self.next_extra;
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
            self.active = Some((extra, job, Instant::now()));
        }
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
        engine.active = Some((
            99,
            Job::Audio {
                chat_id: 42,
                before: 0,
                epoch: 0,
            },
            Instant::now(),
        ));
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
}

use super::TdJson;
use crate::adapters::sqlite::SourceStore;
use music_core::domain::{DiscoveryStage, SourceChat, SourceCommand, SourceKind, SourceSnapshot};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::watch;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Clone)]
enum Job {
    LoadMain,
    LoadArchive,
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
        bot_id: i64,
        before: i64,
    },
    Document {
        chat_id: i64,
        bot_id: i64,
        before: i64,
    },
}

#[derive(Clone)]
struct ChatMeta {
    chat_id: i64,
    title: String,
    private_user: Option<i64>,
    supergroup: Option<i64>,
}

pub struct SourceEngine {
    store: Option<SourceStore>,
    snapshot: SourceSnapshot,
    updates: watch::Sender<SourceSnapshot>,
    chats: BTreeMap<i64, ChatMeta>,
    jobs: VecDeque<Job>,
    active: Option<(i64, Job, Instant)>,
    next_extra: i64,
    logout_account: Option<i64>,
    initial_complete: bool,
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
            jobs: VecDeque::new(),
            active: None,
            next_extra: 1000,
            logout_account: None,
            initial_complete: false,
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
        self.add_chat(SourceChat {
            chat_id: user_id,
            title: "Saved Messages".into(),
            subtitle: "Your personal Telegram archive".into(),
            kind: SourceKind::SavedMessages,
            selected: false,
        });
        let cached: Vec<_> = self.chats.values().cloned().collect();
        for chat in cached {
            if chat.chat_id == user_id {
                continue;
            }
            self.add_chat(SourceChat {
                chat_id: chat.chat_id,
                title: chat.title,
                subtitle: "Telegram chat".into(),
                kind: SourceKind::OtherChat,
                selected: false,
            });
        }
        if let Some(store) = &self.store {
            match store.selected_chats(user_id) {
                Ok(chats) => {
                    for chat in chats {
                        if chat.chat_id != user_id {
                            self.add_chat(chat);
                        }
                    }
                }
                Err(_) => {
                    self.fail("Can't read saved sources. Check device storage and retry.");
                    return;
                }
            }
        } else {
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
        self.snapshot.stage = DiscoveryStage::LoadingChats;
        self.snapshot.checked_bots = 0;
        self.snapshot.total_bots = 0;
        self.snapshot.failed_checks = 0;
        self.snapshot.error = None;
        self.snapshot.show_partial = false;
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
                    .find(|chat| chat.chat_id == chat_id)
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
        let is_new = !self.chats.contains_key(&chat_id);
        self.chats.insert(
            chat_id,
            ChatMeta {
                chat_id,
                title: title.clone(),
                private_user,
                supergroup,
            },
        );
        if self.snapshot.account_id.is_none() {
            return;
        }
        let existing = self
            .snapshot
            .chats
            .iter()
            .find(|item| item.chat_id == chat_id)
            .cloned();
        self.add_chat(SourceChat {
            chat_id,
            title,
            subtitle: existing
                .as_ref()
                .filter(|item| item.kind != SourceKind::OtherChat)
                .map(|item| item.subtitle.clone())
                .unwrap_or_else(|| {
                    match kind {
                        "chatTypePrivate" => "Private chat",
                        "chatTypeSupergroup" => "Channel or group",
                        "chatTypeBasicGroup" => "Group chat",
                        _ => "Telegram chat",
                    }
                    .into()
                }),
            kind: existing.map_or(SourceKind::OtherChat, |item| item.kind),
            selected: false,
        });
        if is_new
            && matches!(
                self.snapshot.stage,
                DiscoveryStage::CheckingBots | DiscoveryStage::Complete
            )
        {
            if let Some(user_id) = private_user {
                self.jobs.push_back(Job::User { chat_id, user_id });
            }
            if let Some(supergroup_id) = supergroup {
                self.jobs.push_back(Job::Channel {
                    chat_id,
                    supergroup_id,
                });
            }
            if !self.jobs.is_empty() {
                if self.initial_complete {
                    self.snapshot.show_partial = true;
                }
                self.snapshot.stage = DiscoveryStage::CheckingBots;
                self.publish();
            }
        }
    }

    pub fn handle_value(&mut self, _td: &TdJson, value: &Value) -> bool {
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
        if value["@type"] == "error" {
            if value["code"] == 404 && matches!(job, Job::LoadMain | Job::LoadArchive) {
                self.end_chat_list(job);
            } else {
                let code = value["code"].as_i64().unwrap_or(0);
                let job_name = match job {
                    Job::LoadMain | Job::LoadArchive => "chat list",
                    Job::User { .. } => "user",
                    Job::Channel { .. } => "channel",
                    Job::Audio { .. } | Job::Document { .. } => "bot history",
                };
                eprintln!("TuneStash source discovery: {job_name} request failed with code {code}");
                if matches!(job, Job::LoadMain | Job::LoadArchive) {
                    self.fail("Couldn't finish loading Telegram chats. Retry or continue with the chats found so far.");
                } else {
                    self.snapshot.failed_checks += 1;
                    if matches!(job, Job::Audio { .. } | Job::Document { .. }) {
                        self.snapshot.checked_bots += 1;
                    }
                    self.snapshot.error = Some(format!(
                        "{} chats could not be checked. You can retry discovery.",
                        self.snapshot.failed_checks
                    ));
                    self.publish();
                }
            }
            return true;
        }
        match job {
            Job::LoadMain | Job::LoadArchive => {
                self.jobs.push_front(job);
            }
            Job::User { chat_id, user_id } => {
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
                    self.snapshot.total_bots += 1;
                    self.jobs.push_back(Job::Audio {
                        chat_id,
                        bot_id: user_id,
                        before: 0,
                    });
                }
                self.publish();
            }
            Job::Channel { chat_id, .. } => {
                if value["is_channel"] == true
                    && value["status"]["@type"] == "chatMemberStatusCreator"
                {
                    self.set_kind(chat_id, SourceKind::PersonalChannel, "Your channel");
                }
            }
            Job::Audio {
                chat_id,
                bot_id,
                before,
            } => {
                self.process_probe(value, chat_id, bot_id, before, false);
            }
            Job::Document {
                chat_id,
                bot_id,
                before,
            } => {
                self.process_probe(value, chat_id, bot_id, before, true);
            }
        }
        true
    }

    fn end_chat_list(&mut self, job: Job) {
        match job {
            Job::LoadMain => self.jobs.push_front(Job::LoadArchive),
            Job::LoadArchive => {
                self.snapshot.stage = DiscoveryStage::CheckingBots;
                for chat in self.chats.values() {
                    if let Some(user_id) = chat.private_user {
                        self.jobs.push_back(Job::User {
                            chat_id: chat.chat_id,
                            user_id,
                        });
                    }
                    if let Some(supergroup_id) = chat.supergroup {
                        self.jobs.push_back(Job::Channel {
                            chat_id: chat.chat_id,
                            supergroup_id,
                        });
                    }
                }
                self.publish();
            }
            _ => (),
        }
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

    fn process_probe(
        &mut self,
        value: &Value,
        chat_id: i64,
        bot_id: i64,
        before: i64,
        documents: bool,
    ) {
        let messages = value["messages"].as_array();
        let found = messages.is_some_and(|messages| {
            messages.iter().any(|message| {
                message["content"]["@type"]
                    == if documents {
                        "messageDocument"
                    } else {
                        "messageAudio"
                    }
                    && super::bot_music_message(message, bot_id)
            })
        });
        if found {
            self.set_kind(chat_id, SourceKind::MusicBot, "Music bot");
            self.snapshot.checked_bots += 1;
            self.publish();
            return;
        }
        let last = messages
            .and_then(|items| items.last())
            .and_then(|item| item["id"].as_i64());
        if let Some(last) = last.filter(|id| *id > 1 && (before == 0 || *id < before)) {
            let next = if documents {
                Job::Document {
                    chat_id,
                    bot_id,
                    before: last - 1,
                }
            } else {
                Job::Audio {
                    chat_id,
                    bot_id,
                    before: last - 1,
                }
            };
            self.jobs.push_back(next);
        } else if documents {
            self.snapshot.checked_bots += 1;
            self.publish();
        } else {
            self.jobs.push_back(Job::Document {
                chat_id,
                bot_id,
                before: 0,
            });
        }
    }

    pub fn drive(&mut self, td: &TdJson) {
        if self.logout_account.is_some() || self.snapshot.account_id.is_none() {
            return;
        }
        if let Some((_, _, started)) = &self.active {
            if started.elapsed() > REQUEST_TIMEOUT {
                self.fail("Telegram took too long to check chats. Retry or continue with the chats found so far.");
            }
            return;
        }
        let Some(job) = self.jobs.pop_front() else {
            if self.snapshot.stage == DiscoveryStage::CheckingBots {
                self.snapshot.stage = DiscoveryStage::Complete;
                self.initial_complete = true;
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
            self.fail("Couldn't request Telegram chats. Check your connection and retry.");
        } else {
            self.active = Some((extra, job, Instant::now()));
        }
    }
}

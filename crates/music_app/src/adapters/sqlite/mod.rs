//! Account-scoped source choices. This connection is owned by the service
//! thread; Dioxus never reads SQLite while rendering.

use music_core::domain::{MusicCheck, SourceChat, SourceKind};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

fn kind_name(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::SavedMessages => "saved",
        SourceKind::PersonalChannel => "channel",
        SourceKind::MusicBot => "bot",
        SourceKind::OtherChat => "chat",
    }
}

fn parse_kind(kind: &str) -> SourceKind {
    match kind {
        "saved" => SourceKind::SavedMessages,
        "channel" => SourceKind::PersonalChannel,
        "bot" => SourceKind::MusicBot,
        _ => SourceKind::OtherChat,
    }
}

fn music_name(music: MusicCheck) -> &'static str {
    match music {
        MusicCheck::Unchecked => "unchecked",
        MusicCheck::Checking => "checking",
        MusicCheck::Found => "found",
        MusicCheck::Empty => "empty",
        MusicCheck::Failed => "failed",
    }
}

fn parse_music(music: &str) -> MusicCheck {
    match music {
        "found" => MusicCheck::Found,
        "empty" => MusicCheck::Empty,
        "failed" => MusicCheck::Failed,
        _ => MusicCheck::Unchecked,
    }
}

pub struct SourceStore {
    connection: Connection,
}

impl SourceStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 BEGIN IMMEDIATE;
                 CREATE TABLE IF NOT EXISTS source_choices (
                   id INTEGER PRIMARY KEY,
                   account_id INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL,
                   kind TEXT NOT NULL,
                   display_name TEXT NOT NULL,
                   enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                   UNIQUE(account_id, chat_id)
                 );
                 CREATE TABLE IF NOT EXISTS source_scan_state (
                   account_id INTEGER PRIMARY KEY,
                   completed_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS discovered_source_chats (
                   account_id INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL,
                   title TEXT NOT NULL,
                   subtitle TEXT NOT NULL,
                   kind TEXT NOT NULL,
                   music TEXT NOT NULL,
                   PRIMARY KEY(account_id, chat_id),
                   FOREIGN KEY(account_id) REFERENCES source_scan_state(account_id)
                     ON DELETE CASCADE
                 );
                 PRAGMA user_version = 1;
                 COMMIT;",
            )
            .map_err(|error| error.to_string())?;
        Ok(Self { connection })
    }

    pub fn selected(&self, account_id: i64, chat_id: i64) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT enabled FROM source_choices WHERE account_id = ?1 AND chat_id = ?2",
                params![account_id, chat_id],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map(|value| value.unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    pub fn set_selected(
        &self,
        account_id: i64,
        chat: &SourceChat,
        selected: bool,
    ) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO source_choices(account_id, chat_id, kind, display_name, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(account_id, chat_id) DO UPDATE SET
                   kind = excluded.kind, display_name = excluded.display_name,
                   enabled = excluded.enabled",
                params![
                    account_id,
                    chat.chat_id,
                    kind_name(chat.kind),
                    chat.title,
                    selected
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub fn selected_chats(&self, account_id: i64) -> Result<Vec<SourceChat>, String> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chat_id, display_name, kind FROM source_choices
             WHERE account_id = ?1 AND enabled = 1 ORDER BY display_name, chat_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([account_id], |row| {
                let kind: String = row.get(2)?;
                let kind = parse_kind(&kind);
                Ok(SourceChat {
                    chat_id: row.get(0)?,
                    title: row.get(1)?,
                    subtitle: match kind {
                        SourceKind::SavedMessages => "Your personal Telegram archive",
                        SourceKind::PersonalChannel => "Your channel",
                        SourceKind::MusicBot => "Music bot",
                        SourceKind::OtherChat => "Waiting for Telegram chat details",
                    }
                    .into(),
                    kind,
                    selected: true,
                    music: MusicCheck::Unchecked,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    /// A marker row and its chat catalog are committed together. An interrupted
    /// scan cannot make a partial catalog look like a completed discovery.
    pub fn save_discovery(&mut self, account_id: i64, chats: &[SourceChat]) -> Result<(), String> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT INTO source_scan_state(account_id, completed_at)
                 VALUES (?1, unixepoch())
                 ON CONFLICT(account_id) DO UPDATE SET completed_at = excluded.completed_at",
                [account_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM discovered_source_chats WHERE account_id = ?1",
                [account_id],
            )
            .map_err(|error| error.to_string())?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO discovered_source_chats
                     (account_id, chat_id, title, subtitle, kind, music)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(|error| error.to_string())?;
            for chat in chats {
                statement
                    .execute(params![
                        account_id,
                        chat.chat_id,
                        chat.title,
                        chat.subtitle,
                        kind_name(chat.kind),
                        music_name(chat.music)
                    ])
                    .map_err(|error| error.to_string())?;
            }
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn load_discovery(&self, account_id: i64) -> Result<Option<Vec<SourceChat>>, String> {
        let completed = self
            .connection
            .query_row(
                "SELECT 1 FROM source_scan_state WHERE account_id = ?1",
                [account_id],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some();
        if !completed {
            return Ok(None);
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT chats.chat_id, chats.title, chats.subtitle, chats.kind,
                        chats.music, COALESCE(choices.enabled, 0)
                 FROM discovered_source_chats AS chats
                 LEFT JOIN source_choices AS choices
                   ON choices.account_id = chats.account_id
                  AND choices.chat_id = chats.chat_id
                 WHERE chats.account_id = ?1 ORDER BY chats.title, chats.chat_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([account_id], |row| {
                let kind: String = row.get(3)?;
                let music: String = row.get(4)?;
                Ok(SourceChat {
                    chat_id: row.get(0)?,
                    title: row.get(1)?,
                    subtitle: row.get(2)?,
                    kind: parse_kind(&kind),
                    music: parse_music(&music),
                    selected: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub fn clear_account(&mut self, account_id: i64) -> Result<(), String> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM source_scan_state WHERE account_id = ?1",
                [account_id],
            )
            .map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM source_choices WHERE account_id = ?1",
                [account_id],
            )
            .map_err(|error| error.to_string())?;
        transaction.commit().map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_survive_reopen_and_are_scoped_to_account() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-source-test-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let store = SourceStore::open(&path).unwrap();
            let chat = SourceChat {
                chat_id: 99,
                title: "Bot".into(),
                subtitle: String::new(),
                kind: SourceKind::MusicBot,
                selected: true,
                music: MusicCheck::Found,
            };
            store.set_selected(10, &chat, true).unwrap();
            store.set_selected(10, &chat, true).unwrap();
        }
        let mut store = SourceStore::open(&path).unwrap();
        assert!(store.selected(10, 99).unwrap());
        assert_eq!(
            store.selected_chats(10).unwrap()[0].kind,
            SourceKind::MusicBot
        );
        assert!(!store.selected(11, 99).unwrap());
        let chat = store.selected_chats(10).unwrap().remove(0);
        store.set_selected(10, &chat, false).unwrap();
        assert!(!store.selected(10, 99).unwrap());
        assert!(store.selected_chats(10).unwrap().is_empty());
        store.clear_account(10).unwrap();
        assert!(!store.selected(10, 99).unwrap());
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn populated_choice_database_upgrades_and_discovery_is_atomic() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-source-upgrade-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let old = Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE source_choices (
               id INTEGER PRIMARY KEY,
               account_id INTEGER NOT NULL,
               chat_id INTEGER NOT NULL,
               kind TEXT NOT NULL,
               display_name TEXT NOT NULL,
               enabled INTEGER NOT NULL,
               UNIQUE(account_id, chat_id)
             );
             INSERT INTO source_choices(account_id, chat_id, kind, display_name, enabled)
             VALUES (10, 99, 'bot', 'Saved bot', 1);",
        )
        .unwrap();
        drop(old);

        let mut store = SourceStore::open(&path).unwrap();
        assert!(store.selected(10, 99).unwrap());
        assert!(store.load_discovery(10).unwrap().is_none());
        let chat = SourceChat {
            chat_id: 99,
            title: "Saved bot".into(),
            subtitle: "Bot conversation".into(),
            kind: SourceKind::MusicBot,
            selected: true,
            music: MusicCheck::Found,
        };
        store
            .save_discovery(10, std::slice::from_ref(&chat))
            .unwrap();
        assert!(
            store
                .save_discovery(10, &[chat.clone(), chat.clone()])
                .is_err()
        );
        drop(store);

        let mut reopened = SourceStore::open(&path).unwrap();
        assert_eq!(reopened.load_discovery(10).unwrap(), Some(vec![chat]));
        assert!(reopened.load_discovery(11).unwrap().is_none());
        reopened.clear_account(10).unwrap();
        assert!(reopened.load_discovery(10).unwrap().is_none());
        assert!(!reopened.selected(10, 99).unwrap());
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }
}

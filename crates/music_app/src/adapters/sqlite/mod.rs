//! Account-scoped source choices. This connection is owned by the service
//! thread; Dioxus never reads SQLite while rendering.

use music_core::domain::{SourceChat, SourceKind};
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

pub struct SourceStore {
    connection: Connection,
}

impl SourceStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS source_choices (
                   id INTEGER PRIMARY KEY,
                   account_id INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL,
                   kind TEXT NOT NULL,
                   display_name TEXT NOT NULL,
                   enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                   UNIQUE(account_id, chat_id)
                 );",
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
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    pub fn clear_account(&self, account_id: i64) -> Result<(), String> {
        self.connection
            .execute(
                "DELETE FROM source_choices WHERE account_id = ?1",
                [account_id],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
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
            };
            store.set_selected(10, &chat, true).unwrap();
            store.set_selected(10, &chat, true).unwrap();
        }
        let store = SourceStore::open(&path).unwrap();
        assert!(store.selected(10, 99).unwrap());
        assert_eq!(
            store.selected_chats(10).unwrap()[0].kind,
            SourceKind::MusicBot
        );
        assert!(!store.selected(11, 99).unwrap());
        store.clear_account(10).unwrap();
        assert!(!store.selected(10, 99).unwrap());
        drop(store);
        let _ = std::fs::remove_file(path);
    }
}

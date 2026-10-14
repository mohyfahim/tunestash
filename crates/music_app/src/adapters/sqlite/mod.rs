//! Account-scoped source choices. This connection is owned by the service
//! thread; Dioxus never reads SQLite while rendering.

pub mod library;
pub mod playback;

use music_core::application::normalize_search;
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

fn migrate_storage(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare("PRAGMA table_info(saved_audio)")
        .map_err(|error| error.to_string())?;
    let columns: Vec<String> = statement
        .query_map([], |row| row.get(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .map_err(|error| error.to_string())?;
    if !columns.iter().any(|column| column == "title") {
        connection
            .execute_batch("ALTER TABLE saved_audio ADD COLUMN title TEXT NOT NULL DEFAULT '';")
            .map_err(|error| error.to_string())?;
    }
    if !columns.iter().any(|column| column == "artist") {
        connection
            .execute_batch("ALTER TABLE saved_audio ADD COLUMN artist TEXT NOT NULL DEFAULT '';")
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute_batch(
            "UPDATE saved_audio SET
               title = COALESCE((SELECT title FROM indexed_tracks AS t
                 WHERE t.account_id=saved_audio.account_id AND t.chat_id=saved_audio.chat_id
                   AND t.message_id=saved_audio.message_id), ''),
               artist = COALESCE((SELECT artist FROM indexed_tracks AS t
                 WHERE t.account_id=saved_audio.account_id AND t.chat_id=saved_audio.chat_id
                   AND t.message_id=saved_audio.message_id), '');
             UPDATE audio_downloads SET status='failed', error='Retry save after update'
               WHERE status IN ('queued','downloading');
             PRAGMA user_version = 6;
             COMMIT;",
        )
        .map_err(|error| error.to_string())
}

fn migrate_search(connection: &Connection) -> Result<(), String> {
    let columns: Vec<String> = {
        let mut statement = connection
            .prepare("PRAGMA table_info(indexed_tracks)")
            .map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| row.get(1))
            .map_err(|error| error.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|error| error.to_string())?
    };
    connection
        .execute_batch("BEGIN IMMEDIATE;")
        .map_err(|error| error.to_string())?;
    for column in ["search_title", "search_artist", "search_filename"] {
        if !columns.iter().any(|existing| existing == column) {
            connection
                .execute_batch(&format!(
                    "ALTER TABLE indexed_tracks ADD COLUMN {column} TEXT NOT NULL DEFAULT '';"
                ))
                .map_err(|error| error.to_string())?;
        }
    }
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS recent_searches (
           account_id INTEGER NOT NULL,
           normalized_query TEXT NOT NULL,
           display_query TEXT NOT NULL,
           used_order INTEGER NOT NULL,
           PRIMARY KEY(account_id, normalized_query)
         );",
        )
        .map_err(|error| error.to_string())?;
    let rows = {
        let mut statement = connection.prepare(
            "SELECT account_id, chat_id, message_id, title, artist, filename FROM indexed_tracks"
        ).map_err(|error| error.to_string())?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?
    };
    for (account, chat, message, title, artist, filename) in rows {
        connection
            .execute(
                "UPDATE indexed_tracks SET search_title=?4, search_artist=?5, search_filename=?6
             WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![
                    account,
                    chat,
                    message,
                    normalize_search(&title),
                    normalize_search(&artist),
                    normalize_search(&filename)
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    connection
        .execute_batch("PRAGMA user_version = 7; COMMIT;")
        .map_err(|error| error.to_string())
}

pub struct SourceStore {
    connection: Connection,
}

impl SourceStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| error.to_string())?;
        if version > 7 {
            return Err("Source database was created by a newer app version.".into());
        }
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
                 CREATE TABLE IF NOT EXISTS source_setup_complete (
                   account_id INTEGER PRIMARY KEY
                 );
                 CREATE TABLE IF NOT EXISTS indexed_tracks (
                   account_id INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL,
                   message_id INTEGER NOT NULL,
                   media_type TEXT NOT NULL,
                   message_date INTEGER NOT NULL,
                   title TEXT NOT NULL,
                   artist TEXT NOT NULL,
                   filename TEXT NOT NULL,
                   duration_seconds INTEGER,
                   cover_data TEXT,
                   scan_generation INTEGER NOT NULL,
                   PRIMARY KEY(account_id, chat_id, message_id)
                 );
                 CREATE INDEX IF NOT EXISTS indexed_tracks_recent
                   ON indexed_tracks(account_id, message_date DESC, chat_id DESC, message_id DESC);
                 CREATE TABLE IF NOT EXISTS library_scan_state (
                   account_id INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL,
                   generation INTEGER NOT NULL DEFAULT 1,
                   audio_cursor INTEGER NOT NULL DEFAULT 0,
                   document_cursor INTEGER NOT NULL DEFAULT 0,
                   audio_done INTEGER NOT NULL DEFAULT 0,
                   document_done INTEGER NOT NULL DEFAULT 0,
                   PRIMARY KEY(account_id, chat_id)
                 );
                 CREATE TABLE IF NOT EXISTS favorite_tracks (
                   account_id INTEGER NOT NULL, chat_id INTEGER NOT NULL,
                   message_id INTEGER NOT NULL,
                   PRIMARY KEY(account_id, chat_id, message_id)
                 );
                 CREATE TABLE IF NOT EXISTS saved_audio (
                   account_id INTEGER NOT NULL, chat_id INTEGER NOT NULL,
                   message_id INTEGER NOT NULL, path TEXT NOT NULL,
                   size_bytes INTEGER NOT NULL,
                   PRIMARY KEY(account_id, chat_id, message_id)
                 );
                 CREATE TABLE IF NOT EXISTS audio_downloads (
                   account_id INTEGER NOT NULL, chat_id INTEGER NOT NULL,
                   message_id INTEGER NOT NULL, status TEXT NOT NULL,
                   error TEXT,
                   PRIMARY KEY(account_id, chat_id, message_id)
                 );
                 CREATE TABLE IF NOT EXISTS playback_state (
                   account_id INTEGER PRIMARY KEY, cursor INTEGER NOT NULL,
                   position_ms INTEGER NOT NULL, shuffle INTEGER NOT NULL,
                   repeat_mode TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS playback_queue (
                   account_id INTEGER NOT NULL, slot INTEGER NOT NULL,
                   chat_id INTEGER NOT NULL, message_id INTEGER NOT NULL,
                   title TEXT NOT NULL, artist TEXT NOT NULL, filename TEXT NOT NULL,
                   duration_seconds INTEGER, cover_data TEXT, play_order INTEGER NOT NULL,
                   PRIMARY KEY(account_id, slot)
                 );
                 CREATE TABLE IF NOT EXISTS storage_settings (
                   account_id INTEGER PRIMARY KEY,
                   stream_only INTEGER NOT NULL DEFAULT 0 CHECK(stream_only IN (0,1))
                 );
                 COMMIT;",
            )
            .map_err(|error| error.to_string())?;
        if version < 6 {
            self::migrate_storage(&connection)?;
        }
        if version < 7 {
            self::migrate_search(&connection)?;
        }
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

    pub fn setup_complete(&self, account_id: i64) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT 1 FROM source_setup_complete WHERE account_id = ?1",
                [account_id],
                |_| Ok(()),
            )
            .optional()
            .map(|value| value.is_some())
            .map_err(|error| error.to_string())
    }

    pub fn finish_setup(&self, account_id: i64) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT OR IGNORE INTO source_setup_complete(account_id) VALUES (?1)",
                [account_id],
            )
            .map(|_| ())
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
                "DELETE FROM source_setup_complete WHERE account_id = ?1",
                [account_id],
            )
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
        transaction
            .execute(
                "DELETE FROM indexed_tracks WHERE account_id = ?1",
                [account_id],
            )
            .map_err(|error| error.to_string())?;
        for table in [
            "favorite_tracks",
            "saved_audio",
            "audio_downloads",
            "playback_queue",
            "playback_state",
            "storage_settings",
            "recent_searches",
        ] {
            transaction
                .execute(
                    &format!("DELETE FROM {table} WHERE account_id = ?1"),
                    [account_id],
                )
                .map_err(|error| error.to_string())?;
        }
        transaction
            .execute(
                "DELETE FROM library_scan_state WHERE account_id = ?1",
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

    #[test]
    fn v1_catalog_upgrades_without_losing_sources_and_setup_is_account_scoped() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-source-setup-upgrade-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let chat = SourceChat {
            chat_id: 99,
            title: "Music bot".into(),
            subtitle: "Bot conversation".into(),
            kind: SourceKind::MusicBot,
            selected: true,
            music: MusicCheck::Found,
        };
        let mut old = SourceStore::open(&path).unwrap();
        old.set_selected(10, &chat, true).unwrap();
        old.save_discovery(10, std::slice::from_ref(&chat)).unwrap();
        drop(old);
        let old_schema = Connection::open(&path).unwrap();
        old_schema
            .execute_batch("DROP TABLE source_setup_complete; PRAGMA user_version = 1;")
            .unwrap();
        drop(old_schema);

        let upgraded = SourceStore::open(&path).unwrap();
        assert_eq!(upgraded.load_discovery(10).unwrap(), Some(vec![chat]));
        assert!(!upgraded.setup_complete(10).unwrap());
        upgraded.finish_setup(10).unwrap();
        upgraded.finish_setup(10).unwrap();
        assert!(upgraded.setup_complete(10).unwrap());
        assert!(!upgraded.setup_complete(11).unwrap());
        upgraded.finish_setup(11).unwrap();
        drop(upgraded);

        let mut reopened = SourceStore::open(&path).unwrap();
        assert!(reopened.setup_complete(10).unwrap());
        reopened.clear_account(10).unwrap();
        assert!(!reopened.setup_complete(10).unwrap());
        assert!(reopened.setup_complete(11).unwrap());
        drop(reopened);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn storage_upgrade_does_not_resume_ambiguous_old_streams() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-storage-upgrade-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        SourceStore::open(&path).unwrap();
        let old = Connection::open(&path).unwrap();
        old.execute(
            "INSERT INTO audio_downloads(account_id,chat_id,message_id,status,error)
            VALUES(1,2,3,'downloading',NULL)",
            [],
        )
        .unwrap();
        old.execute_batch("PRAGMA user_version=5;").unwrap();
        drop(old);
        SourceStore::open(&path).unwrap();
        let upgraded = Connection::open(&path).unwrap();
        let status: String = upgraded
            .query_row(
                "SELECT status FROM audio_downloads WHERE account_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "failed");
        let version: i64 = upgraded
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 7);
        drop(upgraded);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn v6_search_upgrade_backfills_existing_tracks() {
        let path = std::env::temp_dir().join(format!(
            "tunestash-search-upgrade-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let old = Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE indexed_tracks (
               account_id INTEGER NOT NULL, chat_id INTEGER NOT NULL, message_id INTEGER NOT NULL,
               media_type TEXT NOT NULL, message_date INTEGER NOT NULL, title TEXT NOT NULL,
               artist TEXT NOT NULL, filename TEXT NOT NULL, duration_seconds INTEGER,
               cover_data TEXT, scan_generation INTEGER NOT NULL,
               PRIMARY KEY(account_id, chat_id, message_id));
             INSERT INTO indexed_tracks VALUES
               (7, 9, 11, 'audio', 1, 'كيان', 'Artist', 'test.mp3', NULL, NULL, 1);
             PRAGMA user_version=6;",
        )
        .unwrap();
        drop(old);
        let source = SourceStore::open(&path).unwrap();
        source
            .set_selected(
                7,
                &SourceChat {
                    chat_id: 9,
                    title: "Archive".into(),
                    subtitle: String::new(),
                    kind: SourceKind::PersonalChannel,
                    selected: true,
                    music: MusicCheck::Found,
                },
                true,
            )
            .unwrap();
        let library = library::LibraryStore::open(&path).unwrap();
        let (results, count) = library.search_page(7, "کیان", 50, 0).unwrap();
        assert_eq!(count, 1);
        assert_eq!(results[0].title, "كيان");
        let version: i64 = Connection::open(&path)
            .unwrap()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 7);
        std::fs::remove_file(path).unwrap();
    }
}

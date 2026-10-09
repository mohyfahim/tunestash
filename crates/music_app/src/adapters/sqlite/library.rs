//! Metadata catalog and resumable source scans. All calls run on the TDLib
//! service thread, never during a Dioxus render.

use music_core::domain::TrackSummary;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaFilter {
    Audio,
    Document,
}

impl MediaFilter {
    pub fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Document => "document",
        }
    }

    fn cursor_column(self) -> &'static str {
        match self {
            Self::Audio => "audio_cursor",
            Self::Document => "document_cursor",
        }
    }

    fn done_column(self) -> &'static str {
        match self {
            Self::Audio => "audio_done",
            Self::Document => "document_done",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackInput {
    pub message_id: i64,
    pub date: i64,
    pub media_type: &'static str,
    pub title: String,
    pub artist: String,
    pub filename: String,
    pub duration_seconds: Option<i64>,
    pub cover_data: Option<String>,
}

pub struct LibraryStore {
    connection: Connection,
}

impl LibraryStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        Connection::open(path)
            .map(|connection| Self { connection })
            .map_err(|error| error.to_string())
    }

    /// Start an unindexed source or resume a partial pass. Only an explicit
    /// refresh restarts a completed pass.
    pub fn start_source(
        &self,
        account: i64,
        chat: i64,
        refresh: bool,
    ) -> Result<Vec<(MediaFilter, i64)>, String> {
        self.connection
            .execute(
                "INSERT OR IGNORE INTO library_scan_state(account_id, chat_id) VALUES (?1, ?2)",
                params![account, chat],
            )
            .map_err(|error| error.to_string())?;
        let (audio_cursor, document_cursor, audio_done, document_done): (i64, i64, bool, bool) =
            self.connection
                .query_row(
                    "SELECT audio_cursor, document_cursor, audio_done, document_done
                 FROM library_scan_state WHERE account_id = ?1 AND chat_id = ?2",
                    params![account, chat],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|error| error.to_string())?;
        if audio_done && document_done {
            if !refresh {
                return Ok(Vec::new());
            }
            self.connection
                .execute(
                    "UPDATE library_scan_state SET generation = generation + 1,
                     audio_cursor = 0, document_cursor = 0,
                     audio_done = 0, document_done = 0
                     WHERE account_id = ?1 AND chat_id = ?2",
                    params![account, chat],
                )
                .map_err(|error| error.to_string())?;
            return Ok(vec![(MediaFilter::Audio, 0), (MediaFilter::Document, 0)]);
        }
        let mut jobs = Vec::with_capacity(2);
        if !audio_done {
            jobs.push((MediaFilter::Audio, audio_cursor));
        }
        if !document_done {
            jobs.push((MediaFilter::Document, document_cursor));
        }
        Ok(jobs)
    }

    /// Store a result page and its cursor atomically. A completed two-filter
    /// pass reconciles messages that disappeared while the app was closed.
    pub fn save_page(
        &mut self,
        account: i64,
        chat: i64,
        filter: MediaFilter,
        next: i64,
        tracks: &[TrackInput],
    ) -> Result<(), String> {
        let transaction = self.connection.transaction().map_err(|e| e.to_string())?;
        let generation: i64 = transaction
            .query_row(
                "SELECT generation FROM library_scan_state WHERE account_id = ?1 AND chat_id = ?2",
                params![account, chat],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO indexed_tracks
                     (account_id, chat_id, message_id, media_type, message_date, title,
                      artist, filename, duration_seconds, cover_data, scan_generation)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(account_id, chat_id, message_id) DO UPDATE SET
                      media_type = excluded.media_type, message_date = excluded.message_date,
                      title = excluded.title, artist = excluded.artist,
                      filename = excluded.filename, duration_seconds = excluded.duration_seconds,
                      cover_data = excluded.cover_data, scan_generation = excluded.scan_generation",
                )
                .map_err(|e| e.to_string())?;
            for track in tracks {
                statement
                    .execute(params![
                        account,
                        chat,
                        track.message_id,
                        track.media_type,
                        track.date,
                        track.title,
                        track.artist,
                        track.filename,
                        track.duration_seconds,
                        track.cover_data,
                        generation
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
        let sql = format!(
            "UPDATE library_scan_state SET {} = ?3, {} = ?4 WHERE account_id = ?1 AND chat_id = ?2",
            filter.cursor_column(),
            filter.done_column()
        );
        transaction
            .execute(&sql, params![account, chat, next, next == 0])
            .map_err(|e| e.to_string())?;
        let complete: bool = transaction
            .query_row(
                "SELECT audio_done AND document_done FROM library_scan_state
                 WHERE account_id = ?1 AND chat_id = ?2",
                params![account, chat],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if complete {
            transaction
                .execute(
                    "DELETE FROM indexed_tracks WHERE account_id = ?1 AND chat_id = ?2
                     AND scan_generation != ?3",
                    params![account, chat, generation],
                )
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    pub fn upsert_live(&self, account: i64, chat: i64, track: &TrackInput) -> Result<(), String> {
        let generation: Option<i64> = self
            .connection
            .query_row(
                "SELECT generation FROM library_scan_state WHERE account_id = ?1 AND chat_id = ?2",
                params![account, chat],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(generation) = generation else {
            return Ok(());
        };
        self.connection
            .execute(
                "INSERT INTO indexed_tracks
             (account_id, chat_id, message_id, media_type, message_date, title, artist,
              filename, duration_seconds, cover_data, scan_generation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(account_id, chat_id, message_id) DO UPDATE SET
              media_type=excluded.media_type, message_date=excluded.message_date,
              title=excluded.title, artist=excluded.artist, filename=excluded.filename,
              duration_seconds=excluded.duration_seconds, cover_data=excluded.cover_data,
              scan_generation=excluded.scan_generation",
                params![
                    account,
                    chat,
                    track.message_id,
                    track.media_type,
                    track.date,
                    track.title,
                    track.artist,
                    track.filename,
                    track.duration_seconds,
                    track.cover_data,
                    generation
                ],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn remove_message(&self, account: i64, chat: i64, message: i64) -> Result<(), String> {
        self.connection.execute(
            "DELETE FROM indexed_tracks WHERE account_id = ?1 AND chat_id = ?2 AND message_id = ?3",
            params![account, chat, message],
        ).map(|_| ()).map_err(|e| e.to_string())
    }

    pub fn page(
        &self,
        account: i64,
        limit: usize,
        initial: Option<char>,
    ) -> Result<(Vec<TrackSummary>, usize), String> {
        let initial = initial.map(|letter| letter.to_string());
        let total: i64 = self
            .connection
            .query_row(
                "SELECT count(*) FROM indexed_tracks AS tracks JOIN source_choices AS choices
             ON choices.account_id = tracks.account_id AND choices.chat_id = tracks.chat_id
             WHERE tracks.account_id = ?1 AND choices.enabled = 1
               AND (?2 IS NULL OR substr(tracks.title, 1, 1) COLLATE NOCASE = ?2)",
                params![account, initial],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT tracks.chat_id, tracks.message_id, tracks.title, tracks.artist,
              tracks.filename, choices.display_name, tracks.message_date,
              tracks.duration_seconds, tracks.cover_data
             FROM indexed_tracks AS tracks JOIN source_choices AS choices
              ON choices.account_id = tracks.account_id AND choices.chat_id = tracks.chat_id
             WHERE tracks.account_id = ?1 AND choices.enabled = 1
               AND (?2 IS NULL OR substr(tracks.title, 1, 1) COLLATE NOCASE = ?2)
             ORDER BY tracks.title COLLATE NOCASE ASC, tracks.chat_id ASC, tracks.message_id ASC
             LIMIT ?3",
            )
            .map_err(|e| e.to_string())?;
        let tracks = statement
            .query_map(params![account, initial, limit as i64], |row| {
                Ok(TrackSummary {
                    chat_id: row.get(0)?,
                    message_id: row.get(1)?,
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    filename: row.get(4)?,
                    source_name: row.get(5)?,
                    date: row.get(6)?,
                    duration_seconds: row.get(7)?,
                    cover_data: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok((tracks, total.max(0) as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite::SourceStore;
    use music_core::domain::{MusicCheck, SourceChat, SourceKind};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture() -> (std::path::PathBuf, LibraryStore) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tunestash-library-{}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let source = SourceStore::open(&path).unwrap();
        source
            .set_selected(
                1,
                &SourceChat {
                    chat_id: 10,
                    title: "Saved Messages".into(),
                    subtitle: String::new(),
                    kind: SourceKind::SavedMessages,
                    selected: true,
                    music: MusicCheck::Found,
                },
                true,
            )
            .unwrap();
        (path.clone(), LibraryStore::open(&path).unwrap())
    }

    fn track(id: i64) -> TrackInput {
        TrackInput {
            message_id: id,
            date: id,
            media_type: "audio",
            title: format!("Song {id}"),
            artist: "Artist".into(),
            filename: format!("{id}.mp3"),
            duration_seconds: Some(60),
            cover_data: None,
        }
    }

    #[test]
    fn pages_resume_without_duplicates_and_reconcile_on_complete_pass() {
        let (path, mut store) = fixture();
        assert_eq!(
            store.start_source(1, 10, false).unwrap(),
            vec![(MediaFilter::Audio, 0), (MediaFilter::Document, 0)]
        );
        store
            .save_page(1, 10, MediaFilter::Audio, 50, &[track(3), track(2)])
            .unwrap();
        drop(store);
        let mut store = LibraryStore::open(&path).unwrap();
        assert_eq!(
            store.start_source(1, 10, false).unwrap(),
            vec![(MediaFilter::Audio, 50), (MediaFilter::Document, 0)]
        );
        store
            .save_page(1, 10, MediaFilter::Audio, 0, &[track(2)])
            .unwrap();
        store
            .save_page(1, 10, MediaFilter::Document, 0, &[])
            .unwrap();
        let (rows, total) = store.page(1, 1, None).unwrap();
        assert_eq!(total, 2);
        assert_eq!(rows[0].message_id, 2);
        assert_eq!(rows[0].source_name, "Saved Messages");
        assert!(store.start_source(1, 10, false).unwrap().is_empty());
        assert_eq!(
            store.start_source(1, 10, true).unwrap(),
            vec![(MediaFilter::Audio, 0), (MediaFilter::Document, 0)]
        );
        store
            .save_page(1, 10, MediaFilter::Audio, 0, &[track(2)])
            .unwrap();
        assert_eq!(store.page(1, 50, None).unwrap().1, 2); // Old rows stay visible during a scan.
        store
            .save_page(1, 10, MediaFilter::Document, 0, &[])
            .unwrap();
        let (rows, total) = store.page(1, 50, None).unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].message_id, 2);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn only_selected_sources_and_current_account_are_visible() {
        let (path, mut store) = fixture();
        store.start_source(1, 10, false).unwrap();
        store
            .save_page(1, 10, MediaFilter::Audio, 0, &[track(1)])
            .unwrap();
        assert_eq!(store.page(1, 50, None).unwrap().1, 1);
        assert_eq!(store.page(2, 50, None).unwrap().1, 0);
        assert_eq!(store.page(1, 50, Some('S')).unwrap().1, 1);
        assert_eq!(store.page(2, 50, Some('S')).unwrap().1, 0);
        let source = SourceStore::open(&path).unwrap();
        source
            .set_selected(
                1,
                &SourceChat {
                    chat_id: 10,
                    title: "Saved Messages".into(),
                    subtitle: String::new(),
                    kind: SourceKind::SavedMessages,
                    selected: false,
                    music: MusicCheck::Found,
                },
                false,
            )
            .unwrap();
        assert_eq!(store.page(1, 50, None).unwrap().1, 0);
        assert_eq!(store.page(1, 50, Some('S')).unwrap().1, 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn title_order_and_letter_filter_are_stable_across_pages() {
        let (path, mut store) = fixture();
        store.start_source(1, 10, false).unwrap();
        let mut tracks = Vec::new();
        for (id, title) in [
            (8, "Beta"),
            (5, "alpha"),
            (2, "Alpha"),
            (9, "آهنگ"),
            (7, "2 Steps"),
        ] {
            let mut item = track(id);
            item.title = title.into();
            tracks.push(item);
        }
        store
            .save_page(1, 10, MediaFilter::Audio, 0, &tracks)
            .unwrap();
        let (first_page, count) = store.page(1, 2, None).unwrap();
        assert_eq!(count, 5);
        assert_eq!(
            first_page
                .iter()
                .map(|row| row.message_id)
                .collect::<Vec<_>>(),
            vec![7, 2]
        );
        let (second_page, count) = store.page(1, 3, None).unwrap();
        assert_eq!(count, 5);
        assert_eq!(second_page[2].message_id, 5);
        let (a_page, a_count) = store.page(1, 1, Some('A')).unwrap();
        assert_eq!(a_count, 2);
        assert_eq!(a_page[0].message_id, 2);
        let (a_page, _) = store.page(1, 2, Some('A')).unwrap();
        assert_eq!(a_page[1].message_id, 5);
        assert_eq!(store.page(1, 50, Some('Z')).unwrap().1, 0);
        assert_eq!(store.page(1, 50, Some('2')).unwrap().1, 1);
        std::fs::remove_file(path).unwrap();
    }
}

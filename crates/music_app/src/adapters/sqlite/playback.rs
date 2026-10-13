use music_core::domain::{PlaybackSnapshot, QueueTrack, RepeatMode, StoredAudio, TrackKey};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

pub struct PlaybackStore {
    connection: Connection,
}

impl PlaybackStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| e.to_string())?;
        connection
            .busy_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        Ok(Self { connection })
    }

    pub fn favorite(&self, key: TrackKey) -> Result<bool, String> {
        self.connection.query_row(
            "SELECT 1 FROM favorite_tracks WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
            params![key.account_id, key.chat_id, key.message_id], |_| Ok(())
        ).optional().map(|row| row.is_some()).map_err(|e| e.to_string())
    }

    pub fn toggle_favorite(&self, key: TrackKey) -> Result<bool, String> {
        if self.favorite(key)? {
            self.connection.execute("DELETE FROM favorite_tracks WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![key.account_id, key.chat_id, key.message_id]).map_err(|e| e.to_string())?;
            Ok(false)
        } else {
            self.connection
                .execute(
                    "INSERT INTO favorite_tracks VALUES(?1,?2,?3)",
                    params![key.account_id, key.chat_id, key.message_id],
                )
                .map_err(|e| e.to_string())?;
            Ok(true)
        }
    }

    pub fn saved_path(&self, key: TrackKey) -> Result<Option<String>, String> {
        let saved: Option<(String, i64)> = self.connection.query_row(
            "SELECT path, size_bytes FROM saved_audio WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
            params![key.account_id, key.chat_id, key.message_id], |r| Ok((r.get(0)?, r.get(1)?))
        ).optional().map_err(|e| e.to_string())?;
        match saved {
            Some((path, size)) => match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() && size > 0 && metadata.len() == size as u64 => {
                    Ok(Some(path))
                }
                Ok(_) => {
                    self.connection.execute("DELETE FROM saved_audio WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                        params![key.account_id, key.chat_id, key.message_id]).map_err(|e| e.to_string())?;
                    Ok(None)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    self.connection.execute("DELETE FROM saved_audio WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                        params![key.account_id, key.chat_id, key.message_id]).map_err(|e| e.to_string())?;
                    Ok(None)
                }
                Err(error) => Err(format!("Could not inspect saved audio: {error}")),
            },
            None => Ok(None),
        }
    }

    pub fn save_file(&self, key: TrackKey, path: &Path, size: u64) -> Result<(), String> {
        self.connection.execute(
            "INSERT INTO saved_audio(account_id,chat_id,message_id,path,size_bytes,title,artist)
             VALUES(?1,?2,?3,?4,?5,
               COALESCE((SELECT title FROM indexed_tracks WHERE account_id=?1 AND chat_id=?2 AND message_id=?3),''),
               COALESCE((SELECT artist FROM indexed_tracks WHERE account_id=?1 AND chat_id=?2 AND message_id=?3),''))
             ON CONFLICT(account_id,chat_id,message_id) DO UPDATE SET path=excluded.path,size_bytes=excluded.size_bytes,
               title=excluded.title,artist=excluded.artist",
            params![key.account_id,key.chat_id,key.message_id,path.to_string_lossy(),size as i64]
        ).map_err(|e| e.to_string())?;
        self.connection
            .execute(
                "DELETE FROM audio_downloads WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![key.account_id, key.chat_id, key.message_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn mark_download(
        &self,
        key: TrackKey,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        self.connection.execute("INSERT INTO audio_downloads VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT(account_id,chat_id,message_id) DO UPDATE SET status=excluded.status,error=excluded.error",
            params![key.account_id,key.chat_id,key.message_id,status,error])
            .map(|_| ()).map_err(|e| e.to_string())
    }

    pub fn pending_downloads(&self, account: i64) -> Result<Vec<TrackKey>, String> {
        let mut stmt = self
            .connection
            .prepare(
                "SELECT chat_id,message_id FROM audio_downloads
            WHERE account_id=?1 AND status IN ('queued','downloading')",
            )
            .map_err(|e| e.to_string())?;
        stmt.query_map([account], |row| {
            Ok(TrackKey {
                account_id: account,
                chat_id: row.get(0)?,
                message_id: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }

    pub fn download_status(&self, key: TrackKey) -> Result<Option<String>, String> {
        self.connection
            .query_row(
                "SELECT status FROM audio_downloads
            WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![key.account_id, key.chat_id, key.message_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn stream_only(&self, account: i64) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT stream_only FROM storage_settings WHERE account_id=?1",
                [account],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.unwrap_or(false))
            .map_err(|e| e.to_string())
    }

    pub fn set_stream_only(&self, account: i64, enabled: bool) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT INTO storage_settings(account_id,stream_only) VALUES(?1,?2)
            ON CONFLICT(account_id) DO UPDATE SET stream_only=excluded.stream_only",
                params![account, enabled],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn clear_pending(&self, account: i64) -> Result<(), String> {
        self.connection
            .execute("DELETE FROM audio_downloads WHERE account_id=?1", [account])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn saved_entries(&self, account: i64) -> Result<Vec<(TrackKey, String)>, String> {
        let mut stmt = self
            .connection
            .prepare("SELECT chat_id,message_id,path FROM saved_audio WHERE account_id=?1")
            .map_err(|e| e.to_string())?;
        stmt.query_map([account], |row| {
            Ok((
                TrackKey {
                    account_id: account,
                    chat_id: row.get(0)?,
                    message_id: row.get(1)?,
                },
                row.get(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())
    }

    pub fn delete_saved(&self, key: TrackKey, offline_root: &Path) -> Result<(), String> {
        let path: Option<String> = self
            .connection
            .query_row(
                "SELECT path FROM saved_audio WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![key.account_id, key.chat_id, key.message_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(path) = path {
            let path = Path::new(&path);
            match std::fs::symlink_metadata(path) {
                Ok(_) => {
                    let account_dir = offline_root.join(key.account_id.to_string());
                    let safe_root = account_dir.canonicalize().map_err(|e| e.to_string())?;
                    let safe_path = path.canonicalize().map_err(|e| e.to_string())?;
                    if !safe_path.starts_with(&safe_root) {
                        return Err("Saved audio path is outside this account's storage.".into());
                    }
                    std::fs::remove_file(path)
                        .map_err(|error| format!("Could not remove saved audio: {error}"))?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("Could not inspect saved audio: {error}")),
            }
        }
        self.connection
            .execute(
                "DELETE FROM saved_audio WHERE account_id=?1 AND chat_id=?2 AND message_id=?3",
                params![key.account_id, key.chat_id, key.message_id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn storage_page(
        &self,
        account: i64,
        limit: usize,
    ) -> Result<(u64, u64, Vec<StoredAudio>, bool), String> {
        let (count, bytes): (i64, i64) = self
            .connection
            .query_row(
                "SELECT COUNT(*),COALESCE(SUM(size_bytes),0) FROM saved_audio WHERE account_id=?1",
                [account],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = self
            .connection
            .prepare(
                "SELECT chat_id,message_id,title,artist,size_bytes FROM saved_audio
            WHERE account_id=?1 ORDER BY size_bytes DESC,chat_id,message_id LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![account, limit as i64], |row| {
                Ok(StoredAudio {
                    key: TrackKey {
                        account_id: account,
                        chat_id: row.get(0)?,
                        message_id: row.get(1)?,
                    },
                    title: row.get(2)?,
                    artist: row.get(3)?,
                    size_bytes: row.get::<_, i64>(4)?.max(0) as u64,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok((
            count.max(0) as u64,
            bytes.max(0) as u64,
            rows,
            count as usize > limit,
        ))
    }

    pub fn persist(&mut self, state: &PlaybackSnapshot) -> Result<(), String> {
        let Some(key) = state.current_key() else {
            return Ok(());
        };
        let tx = self.connection.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM playback_queue WHERE account_id=?1",
            [key.account_id],
        )
        .map_err(|e| e.to_string())?;
        for (slot, track) in state.queue.iter().enumerate() {
            let play_order = state
                .order
                .iter()
                .position(|index| *index == slot)
                .unwrap_or(slot);
            tx.execute(
                "INSERT INTO playback_queue VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    key.account_id,
                    slot as i64,
                    track.key.chat_id,
                    track.key.message_id,
                    track.title,
                    track.artist,
                    track.filename,
                    track.duration_seconds,
                    track.cover_data,
                    play_order as i64
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        let repeat = match state.repeat {
            RepeatMode::Off => "off",
            RepeatMode::Queue => "queue",
            RepeatMode::Single => "single",
        };
        tx.execute("INSERT INTO playback_state VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT(account_id) DO UPDATE SET cursor=excluded.cursor,position_ms=excluded.position_ms,
            shuffle=excluded.shuffle,repeat_mode=excluded.repeat_mode",
            params![key.account_id,state.cursor as i64,state.position_ms,state.shuffle,repeat])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn restore(&self, account: i64) -> Result<PlaybackSnapshot, String> {
        let mut state = PlaybackSnapshot::default();
        let row: Option<(i64,i64,bool,String)> = self.connection.query_row(
            "SELECT cursor,position_ms,shuffle,repeat_mode FROM playback_state WHERE account_id=?1",
            [account], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
        ).optional().map_err(|e| e.to_string())?;
        let Some((cursor, position, shuffle, repeat)) = row else {
            return Ok(state);
        };
        let mut stmt = self.connection.prepare("SELECT chat_id,message_id,title,artist,filename,duration_seconds,cover_data,play_order
            FROM playback_queue WHERE account_id=?1 ORDER BY slot").map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([account], |r| {
                Ok((
                    QueueTrack {
                        key: TrackKey {
                            account_id: account,
                            chat_id: r.get(0)?,
                            message_id: r.get(1)?,
                        },
                        title: r.get(2)?,
                        artist: r.get(3)?,
                        filename: r.get(4)?,
                        duration_seconds: r.get(5)?,
                        cover_data: r.get(6)?,
                    },
                    r.get::<_, i64>(7)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut ordered: Vec<_> = rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?;
        state.queue = ordered.iter().map(|(track, _)| track.clone()).collect();
        ordered.sort_by_key(|(_, order)| *order);
        state.order = ordered
            .iter()
            .filter_map(|(track, _)| state.queue.iter().position(|item| item.key == track.key))
            .collect();
        state.cursor = (cursor.max(0) as usize).min(state.order.len().saturating_sub(1));
        state.position_ms = position.max(0);
        state.shuffle = shuffle;
        state.repeat = match repeat.as_str() {
            "queue" => RepeatMode::Queue,
            "single" => RepeatMode::Single,
            _ => RepeatMode::Off,
        };
        // Restoration exposes the queue but never starts playback.
        state.status = music_core::domain::PlaybackStatus::Paused;
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::sqlite::SourceStore;
    use music_core::domain::{PlaybackStatus, QueueTrack};

    fn fixture() -> (std::path::PathBuf, PlaybackStore) {
        let path = std::env::temp_dir().join(format!(
            "tunestash-playback-{}-{}.sqlite",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        SourceStore::open(&path).unwrap();
        let store = PlaybackStore::open(&path).unwrap();
        (path, store)
    }

    #[test]
    fn favorites_and_queue_restore_without_autoplay() {
        let (path, mut store) = fixture();
        let key = TrackKey {
            account_id: 91,
            chat_id: 12,
            message_id: 34,
        };
        assert!(store.toggle_favorite(key).unwrap());
        let track = QueueTrack {
            key,
            title: "Song".into(),
            artist: "Artist".into(),
            filename: "a.mp3".into(),
            duration_seconds: Some(100),
            cover_data: None,
        };
        let mut state = PlaybackSnapshot::default();
        assert!(state.load_queue(vec![track], key));
        state.position_ms = 23_000;
        state.status = PlaybackStatus::Playing;
        store.persist(&state).unwrap();
        drop(store);
        let store = PlaybackStore::open(&path).unwrap();
        assert!(store.favorite(key).unwrap());
        let restored = store.restore(key.account_id).unwrap();
        assert_eq!(restored.current_key(), Some(key));
        assert_eq!(restored.position_ms, 23_000);
        assert_eq!(restored.status, PlaybackStatus::Paused);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_saved_file_is_not_offline_ready() {
        let (path, store) = fixture();
        let key = TrackKey {
            account_id: 93,
            chat_id: 12,
            message_id: 34,
        };
        let missing = std::env::temp_dir().join("tunestash-missing-audio.audio");
        let _ = std::fs::remove_file(&missing);
        store.save_file(key, &missing, 100).unwrap();
        assert!(store.saved_path(key).unwrap().is_none());
        assert!(store.saved_path(key).unwrap().is_none());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn interrupted_download_is_recoverable_and_distinct_from_saved_audio() {
        let (path, store) = fixture();
        let key = TrackKey {
            account_id: 94,
            chat_id: 12,
            message_id: 34,
        };
        store.mark_download(key, "downloading", None).unwrap();
        assert_eq!(store.pending_downloads(key.account_id).unwrap(), vec![key]);
        assert!(store.saved_path(key).unwrap().is_none());
        drop(store);
        let store = PlaybackStore::open(&path).unwrap();
        assert_eq!(store.pending_downloads(key.account_id).unwrap(), vec![key]);
        store.mark_download(key, "failed", Some("network")).unwrap();
        assert!(store.pending_downloads(key.account_id).unwrap().is_empty());
        store.mark_download(key, "queued", None).unwrap();
        assert_eq!(store.pending_downloads(key.account_id).unwrap(), vec![key]);
        let audio = path.with_extension("audio");
        std::fs::write(&audio, b"complete").unwrap();
        store.save_file(key, &audio, 8).unwrap();
        assert!(store.pending_downloads(key.account_id).unwrap().is_empty());
        assert_eq!(
            store.saved_path(key).unwrap(),
            Some(audio.to_string_lossy().into_owned())
        );
        let _ = std::fs::remove_file(audio);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn storage_settings_and_saved_rows_are_account_scoped_and_sorted() {
        let (path, store) = fixture();
        let offline = path.with_extension("offline");
        let account_dir = offline.join("5");
        std::fs::create_dir_all(&account_dir).unwrap();
        let small = TrackKey {
            account_id: 5,
            chat_id: 6,
            message_id: 7,
        };
        let large = TrackKey {
            message_id: 8,
            ..small
        };
        for (key, size) in [(small, 3), (large, 9)] {
            let file = account_dir.join(format!("{}.audio", key.message_id));
            std::fs::write(&file, vec![b'a'; size]).unwrap();
            store.save_file(key, &file, size as u64).unwrap();
        }
        store.set_stream_only(5, true).unwrap();
        assert!(store.stream_only(5).unwrap());
        assert!(!store.stream_only(6).unwrap());
        let (count, bytes, page, more) = store.storage_page(5, 1).unwrap();
        assert_eq!((count, bytes, more), (2, 12, true));
        assert_eq!(page[0].key, large);
        store.delete_saved(large, &offline).unwrap();
        assert!(!account_dir.join("8.audio").exists());
        assert_eq!(store.storage_page(5, 50).unwrap().0, 1);
        drop(store);
        assert!(PlaybackStore::open(&path).unwrap().stream_only(5).unwrap());
        let _ = std::fs::remove_dir_all(offline);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_file_deletion_keeps_its_saved_record() {
        let (path, store) = fixture();
        let offline = path.with_extension("offline");
        let key = TrackKey {
            account_id: 5,
            chat_id: 6,
            message_id: 7,
        };
        let file = offline.join("5/7.audio");
        std::fs::create_dir_all(&file).unwrap();
        store.save_file(key, &file, 1).unwrap();
        assert!(store.delete_saved(key, &offline).is_err());
        assert_eq!(store.storage_page(5, 50).unwrap().0, 1);
        let _ = std::fs::remove_dir_all(offline);
        let _ = std::fs::remove_file(path);
    }
}

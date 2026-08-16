use std::collections::HashSet;
use std::sync::Arc;

use rusqlite::params;

use crate::database::{BatchTrackInput, DbError, PlayTuneDb, TrackRecord};

#[derive(Default)]
pub(crate) struct StringDedupPool {
    set: HashSet<Arc<str>>,
}

impl StringDedupPool {
    pub fn get_or_intern(&mut self, s: &str) -> Arc<str> {
        if s.is_empty() {
            return Arc::from("");
        }
        if let Some(existing) = self.set.get(s) {
            Arc::clone(existing)
        } else {
            let shared: Arc<str> = Arc::from(s);
            self.set.insert(Arc::clone(&shared));
            shared
        }
    }

    pub fn get_or_intern_opt(&mut self, s: Option<&str>) -> Option<Arc<str>> {
        s.filter(|v| !v.is_empty()).map(|v| self.get_or_intern(v))
    }
}

impl PlayTuneDb {
    pub const TRACK_SELECT_COLS: &'static str = "id, path, title, artist, album, duration_secs, duration_str, folder_id, is_favorite, play_count, IFNULL(last_played_at, ''), file_modified, replaygain_track_db, replaygain_album_db, replaygain_track_peak, replaygain_album_peak, ebu_r128_loudness, ebu_r128_peak, lyrics_synced, lyrics_unsynced, rating, track_number";

    pub(crate) fn query_tracks<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<TrackRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(sql)?;
        let mut dedup = StringDedupPool::default();

        let rows = stmt.query_map(params, |row| {
            let fav_int: i32 = row.get(8)?;
            let last_played: String = row.get(10)?;
            let file_modified: i64 = row.get(11).unwrap_or(0);
            let rating: i32 = row.get(20).unwrap_or(0);
            let path_s: String = row.get(1)?;
            let title_s: String = row.get(2)?;
            let artist_s: String = row.get(3)?;
            let album_s: String = row.get(4)?;
            let dur_str_s: String = row.get(6)?;
            let synced_s: Option<String> = row.get(18).unwrap_or(None);
            let unsynced_s: Option<String> = row.get(19).unwrap_or(None);

            Ok((
                row.get::<_, i64>(0)?,
                path_s,
                title_s,
                artist_s,
                album_s,
                row.get::<_, f64>(5)?,
                dur_str_s,
                row.get::<_, Option<i64>>(7)?,
                fav_int != 0,
                row.get::<_, i32>(9)?,
                last_played,
                file_modified,
                row.get::<_, Option<f64>>(12).unwrap_or(None),
                row.get::<_, Option<f64>>(13).unwrap_or(None),
                row.get::<_, Option<f64>>(14).unwrap_or(None),
                row.get::<_, Option<f64>>(15).unwrap_or(None),
                row.get::<_, Option<f64>>(16).unwrap_or(None),
                row.get::<_, Option<f64>>(17).unwrap_or(None),
                synced_s,
                unsynced_s,
                rating,
                row.get::<_, Option<i32>>(21).unwrap_or(None),
            ))
        })?;

        let mut tracks = Vec::new();
        for r in rows {
            let (
                id,
                path_s,
                title_s,
                artist_s,
                album_s,
                duration_secs,
                duration_str_s,
                folder_id,
                is_favorite,
                play_count,
                last_played,
                file_modified,
                replaygain_track_db,
                replaygain_album_db,
                replaygain_track_peak,
                replaygain_album_peak,
                ebu_r128_loudness,
                ebu_r128_peak,
                lyrics_synced_s,
                lyrics_unsynced_s,
                rating,
                track_number,
            ) = r?;

            tracks.push(TrackRecord {
                id,
                path: dedup.get_or_intern(&path_s),
                title: dedup.get_or_intern(&title_s),
                artist: dedup.get_or_intern(&artist_s),
                album: dedup.get_or_intern(&album_s),
                duration_secs,
                duration_str: dedup.get_or_intern(&duration_str_s),
                folder_id,
                is_favorite,
                play_count,
                last_played_at: if last_played.is_empty() {
                    None
                } else {
                    Some(dedup.get_or_intern(&last_played))
                },
                file_modified,
                replaygain_track_db,
                replaygain_album_db,
                replaygain_track_peak,
                replaygain_album_peak,
                ebu_r128_loudness,
                ebu_r128_peak,
                lyrics_synced: dedup.get_or_intern_opt(lyrics_synced_s.as_deref()),
                lyrics_unsynced: dedup.get_or_intern_opt(lyrics_unsynced_s.as_deref()),
                rating,
                track_number,
            });
        }
        Ok(tracks)
    }

    // Tracks
    pub fn add_or_update_track(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        duration_secs: f64,
        duration_str: &str,
        folder_id: Option<i64>,
        file_modified: i64,
    ) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tracks (path, title, artist, album, duration_secs, duration_str, folder_id, file_modified)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                duration_secs = excluded.duration_secs,
                duration_str = excluded.duration_str,
                folder_id = excluded.folder_id,
                file_modified = excluded.file_modified",
            params![path, title, artist, album, duration_secs, duration_str, folder_id, file_modified],
        )?;
        let mut stmt = conn.prepare_cached("SELECT id FROM tracks WHERE path = ?")?;
        let id = stmt.query_row(params![path], |row| row.get(0))?;
        Ok(id)
    }

    pub fn get_all_tracks(&self) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!("SELECT {} FROM tracks ORDER BY title ASC", Self::TRACK_SELECT_COLS),
            params![],
        )
    }

    pub fn get_tracks_by_folder(&self, folder_id: i64) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE folder_id = ? ORDER BY title ASC",
                Self::TRACK_SELECT_COLS
            ),
            params![folder_id],
        )
    }

    pub fn get_tracks_by_album(&self, album: &str) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE album = ? ORDER BY title ASC",
                Self::TRACK_SELECT_COLS
            ),
            params![album],
        )
    }

    pub fn get_tracks_by_artist(&self, artist: &str) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE artist = ? ORDER BY title ASC",
                Self::TRACK_SELECT_COLS
            ),
            params![artist],
        )
    }

    pub fn get_favorite_tracks(&self) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE is_favorite = 1 ORDER BY title ASC",
                Self::TRACK_SELECT_COLS
            ),
            params![],
        )
    }

    pub fn get_recently_played_tracks(&self, limit: usize) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(&format!("SELECT {} FROM tracks WHERE last_played_at IS NOT NULL AND last_played_at != '' ORDER BY last_played_at DESC LIMIT ?", Self::TRACK_SELECT_COLS), params![limit as i64])
    }

    /// Issue 6: only show tracks played 3 or more times.
    pub fn get_most_played_tracks(&self, limit: usize) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE play_count >= 3 ORDER BY play_count DESC LIMIT ?",
                Self::TRACK_SELECT_COLS
            ),
            params![limit as i64],
        )
    }

    pub fn get_track_paths_batch(
        &self,
        ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, String>, DbError> {
        if ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let conn = self.conn.lock();
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!("SELECT id, path FROM tracks WHERE id IN ({})", placeholders.join(","));
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(ids.len());
        for id in ids {
            params.push(Box::new(*id));
        }
        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (id, path): (i64, String) = row?;
            map.insert(id, path);
        }
        Ok(map)
    }

    pub fn update_track_loudness(
        &self,
        track_id: i64,
        rg_track_db: Option<f32>,
        rg_track_peak: Option<f32>,
        rg_album_db: Option<f32>,
        rg_album_peak: Option<f32>,
        ebu_r128_loudness: Option<f32>,
        ebu_r128_peak: Option<f32>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tracks SET
                replaygain_track_db = ?,
                replaygain_track_peak = ?,
                replaygain_album_db = ?,
                replaygain_album_peak = ?,
                ebu_r128_loudness = ?,
                ebu_r128_peak = ?
             WHERE id = ?",
            params![
                rg_track_db.map(|v| v as f64),
                rg_track_peak.map(|v| v as f64),
                rg_album_db.map(|v| v as f64),
                rg_album_peak.map(|v| v as f64),
                ebu_r128_loudness.map(|v| v as f64),
                ebu_r128_peak.map(|v| v as f64),
                track_id
            ],
        )?;
        Ok(())
    }

    pub fn update_track_lyrics(
        &self,
        track_id: i64,
        synced: Option<&str>,
        unsynced: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE tracks SET lyrics_synced = ?, lyrics_unsynced = ? WHERE id = ?",
            params![synced, unsynced, track_id],
        )?;
        Ok(())
    }

    pub fn add_or_update_track_with_lyrics(
        &self,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        duration_secs: f64,
        duration_str: &str,
        folder_id: Option<i64>,
        file_modified: i64,
        synced: Option<&str>,
        unsynced: Option<&str>,
        track_number: Option<i32>,
    ) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO tracks (path, title, artist, album, duration_secs, duration_str, folder_id, file_modified, lyrics_synced, lyrics_unsynced, track_number)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                duration_secs = excluded.duration_secs,
                duration_str = excluded.duration_str,
                folder_id = excluded.folder_id,
                file_modified = excluded.file_modified,
                lyrics_synced = COALESCE(excluded.lyrics_synced, tracks.lyrics_synced),
                lyrics_unsynced = COALESCE(excluded.lyrics_unsynced, tracks.lyrics_unsynced),
                track_number = excluded.track_number",
            params![path, title, artist, album, duration_secs, duration_str, folder_id, file_modified, synced, unsynced, track_number],
        )?;
        let mut stmt = conn.prepare_cached("SELECT id FROM tracks WHERE path = ?")?;
        let id = stmt.query_row(params![path], |row| row.get(0))?;
        Ok(id)
    }

    /// Insert or update a batch of tracks inside a **caller-supplied**
    /// `rusqlite::Transaction`. Callers must wrap multiple calls (or a loop)
    /// in a single transaction via `with_transaction` to avoid per-row fsync.
    ///
    /// Returns a Vec of `(original_index, track_id)` pairs so cover-art can
    /// be associated with the correct DB id after the batch commits.
    ///
    /// # Why a separate tx-aware variant?
    ///
    /// `add_or_update_track_with_lyrics` acquires the mutex and commits once
    /// per call. For a 250-track batch on HDD that means 250 WAL syncs
    /// (≈ 5 ms each = 1.25 s of blocking I/O). By sharing a single
    /// transaction, the entire batch flushes in one sync (< 50 ms).
    pub fn insert_tracks_batch_tx<'tx>(
        tx: &rusqlite::Transaction<'tx>,
        tracks: &[BatchTrackInput<'_>],
    ) -> Result<Vec<(usize, i64)>, DbError> {
        let mut ids: Vec<(usize, i64)> = Vec::with_capacity(tracks.len());
        // Prepare once, execute N times — avoids re-parsing the SQL
        // statement on every row.
        let mut insert_stmt = tx.prepare_cached(
            "INSERT INTO tracks \
             (path, title, artist, album, duration_secs, duration_str, folder_id, file_modified, \
              lyrics_synced, lyrics_unsynced, track_number) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(path) DO UPDATE SET \
                title = excluded.title, \
                artist = excluded.artist, \
                album = excluded.album, \
                duration_secs = excluded.duration_secs, \
                duration_str = excluded.duration_str, \
                folder_id = excluded.folder_id, \
                file_modified = excluded.file_modified, \
                lyrics_synced = COALESCE(excluded.lyrics_synced, tracks.lyrics_synced), \
                lyrics_unsynced = COALESCE(excluded.lyrics_unsynced, tracks.lyrics_unsynced), \
                track_number = excluded.track_number",
        )?;
        let mut select_stmt = tx.prepare_cached("SELECT id FROM tracks WHERE path = ?")?;
        for (i, t) in tracks.iter().enumerate() {
            insert_stmt.execute(params![t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8, t.9, t.10])?;
            let id: i64 = select_stmt.query_row(params![t.0], |row| row.get(0))?;
            ids.push((i, id));
        }
        Ok(ids)
    }

    /// Toggle the favorite flag of a track atomically.
    pub fn toggle_favorite(&self, track_id: i64) -> Result<bool, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = ?",
            params![track_id],
        )?;
        let new_state: i32 = tx.query_row(
            "SELECT is_favorite FROM tracks WHERE id = ?",
            params![track_id],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(new_state != 0)
    }

    /// Record a play event. Returns the number of rows affected so the caller
    /// can detect a stale track_id.
    pub fn record_play(&self, track_id: i64) -> Result<usize, DbError> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![track_id],
        )?;
        if affected == 0 {
            log::warn!("record_play: track id {} does not exist (0 rows affected)", track_id);
        }
        Ok(affected)
    }

    /// Remove tracks whose backing file no longer exists on disk
    /// (leftover mock/demo entries or files deleted outside the app).
    /// Returns the number of removed rows.
    pub fn delete_mock_tracks(&self) -> Result<usize, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT id, path FROM tracks")?;
        let rows =
            stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
        let mut stale_ids = Vec::new();
        for row in rows {
            let (id, path) = row?;
            if !std::path::Path::new(&path).is_file() {
                stale_ids.push(id);
            }
        }
        let removed = stale_ids.len();
        for chunk in stale_ids.chunks(500) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM tracks WHERE id IN ({})", placeholders);
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(chunk.len());
            for id in chunk {
                params_vec.push(Box::new(*id));
            }
            let params_ref: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|b| b.as_ref()).collect();
            conn.execute(&sql, params_ref.as_slice())?;
        }
        Ok(removed)
    }

    /// Return `(id, path, file_modified)` for every track row. Used by the
    /// library scanner to detect which files are new, modified, or removed
    /// without re-probing each file.
    pub fn get_tracks_with_mtime(&self) -> Result<Vec<(i64, String, i64)>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT id, path, file_modified FROM tracks")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Remove tracks whose `path` is not in `existing_paths`. Used by the
    /// scanner to garbage-collect entries for files deleted outside the app.
    /// Returns the number of removed rows.
    ///
    /// `existing_paths` is the set of audio file paths found on disk during
    /// the scan. We use a HashSet for O(1) membership checks instead of
    /// calling `Path::is_file()` per row — which would issue one `stat()`
    /// syscall per tracked file (10 000 stat()s = 100 ms–1 s on HDD).
    pub fn cleanup_missing_tracks(&self, existing_paths: &[&str]) -> Result<usize, DbError> {
        // Build a set of on-disk paths once for O(1) lookups.
        let on_disk: std::collections::HashSet<&str> = existing_paths.iter().copied().collect();
        let conn = self.conn.lock();
        let mut stale_ids: Vec<i64> = Vec::new();
        {
            let mut stmt = conn.prepare_cached("SELECT id, path FROM tracks")?;
            let rows =
                stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?;
            for r in rows {
                let (id, path) = r?;
                // Stale = path was not in current scan batch AND does not exist on disk
                if !on_disk.contains(path.as_str()) && !std::path::Path::new(&path).exists() {
                    stale_ids.push(id);
                }
            }
        }
        let removed = stale_ids.len();
        for chunk in stale_ids.chunks(500) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders = std::iter::repeat_n("?", chunk.len()).collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM tracks WHERE id IN ({})", placeholders);
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(chunk.len());
            for id in chunk {
                params_vec.push(Box::new(*id));
            }
            let params_ref: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|b| b.as_ref()).collect();
            conn.execute(&sql, params_ref.as_slice())?;
        }
        Ok(removed)
    }

    /// Fetch a single track by id. Returns `None` if the row does not exist.
    pub fn get_track(&self, track_id: i64) -> Result<Option<TrackRecord>, DbError> {
        let mut tracks = self.query_tracks(
            &format!("SELECT {} FROM tracks WHERE id = ?", Self::TRACK_SELECT_COLS),
            params![track_id],
        )?;
        Ok(tracks.pop())
    }
}

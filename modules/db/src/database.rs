use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderRecord {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub track_count: i32,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecord {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub duration_str: String,
    pub folder_id: Option<i64>,
    pub is_favorite: bool,
    pub play_count: i32,
    pub last_played_at: Option<String>,
    pub file_modified: i64,
    pub replaygain_track_db: Option<f64>,
    pub replaygain_album_db: Option<f64>,
    pub replaygain_track_peak: Option<f64>,
    pub replaygain_album_peak: Option<f64>,
    pub ebu_r128_loudness: Option<f64>,
    pub ebu_r128_peak: Option<f64>,
    pub lyrics_synced: Option<String>,
    pub lyrics_unsynced: Option<String>,
    /// 0–5 star user rating. 0 means "unrated".
    pub rating: i32,
    pub track_number: Option<i32>,
}

/// A user-defined custom playlist row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistRecord {
    pub id: i64,
    pub name: String,
    pub track_count: i32,
    pub duration_secs: f64,
    pub created_at: String,
}

/// Lightweight album aggregator row used by the Albums view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumRecord {
    /// Stable identifier (smallest track id in the album).
    pub id: i64,
    pub album: String,
    pub album_artist: String,
    pub track_count: i32,
    pub duration_secs: f64,
    pub year: Option<i32>,
}

/// Tuple representation of track fields used for batch insertion in transactions.
pub type BatchTrackInput<'a> = (
    &'a str,         // path
    &'a str,         // title
    &'a str,         // artist
    &'a str,         // album
    f64,             // duration_secs
    &'a str,         // duration_str
    Option<i64>,     // folder_id
    i64,             // file_modified
    Option<&'a str>, // lyrics_synced
    Option<&'a str>, // lyrics_unsynced
    Option<i32>,     // track_number
);

/// Lightweight artist aggregator row used by the Artists view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistRecord {
    /// Stable identifier (smallest track id by the artist).
    pub id: i64,
    pub artist: String,
    pub album_count: i32,
    pub track_count: i32,
}

pub struct PlayTuneDb {
    conn: parking_lot::Mutex<Connection>,
}

/// Check if we can actually create files in `dir` by attempting a temp file write.
fn is_writable(dir: &std::path::Path) -> bool {
    let test_path = dir.join(".playtune_write_test");
    let ok = std::fs::write(&test_path, b"").is_ok();
    let _ = std::fs::remove_file(&test_path);
    ok
}

impl PlayTuneDb {
    pub fn open_default() -> Result<Self, DbError> {
        // Try primary location: OS-specific local data dir (~/.local/share/playtune)
        let db_path = if let Some(mut db_dir) = dirs::data_local_dir() {
            db_dir.push("playtune");
            if std::fs::create_dir_all(&db_dir).is_ok() && is_writable(&db_dir) {
                db_dir.join("playtune.db")
            } else {
                // Fallback 1: OS cache dir (~/.cache/playtune)
                if let Some(mut cache_dir) = dirs::cache_dir() {
                    cache_dir.push("playtune");
                    let _ = std::fs::create_dir_all(&cache_dir);
                    cache_dir.join("playtune.db")
                } else {
                    // Fallback 2: local ./playtune_data (relative to CWD)
                    let local = std::path::PathBuf::from("playtune_data");
                    let _ = std::fs::create_dir_all(&local);
                    local.join("playtune.db")
                }
            }
        } else {
            // No data_local_dir at all — try cache then local
            if let Some(mut cache_dir) = dirs::cache_dir() {
                cache_dir.push("playtune");
                let _ = std::fs::create_dir_all(&cache_dir);
                cache_dir.join("playtune.db")
            } else {
                let local = std::path::PathBuf::from("playtune_data");
                let _ = std::fs::create_dir_all(&local);
                local.join("playtune.db")
            }
        };
        log::info!("Opening database at: {}", db_path.display());
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let _ = conn.pragma_update(None, "cache_size", -16_384);
        let _ = conn.pragma_update(None, "temp_store", 2);
        let _ = conn.pragma_update(None, "mmap_size", 268_435_456);
        let db = Self { conn: parking_lot::Mutex::new(conn) };
        db.init_schema()?;
        db.run_maintenance_reset_if_needed()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let db = Self { conn: parking_lot::Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    pub fn init_schema(&self) -> Result<(), DbError> {
        let conn = self.conn.lock();
        // Enable foreign-key enforcement. SQLite defaults to OFF, which made
        // the ON DELETE CASCADE clause in the `tracks` table inert.
        // Setting it as the first statement of every connection guarantees
        // cascading deletes work regardless of how the connection was opened.
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                track_count INTEGER NOT NULL DEFAULT 0 CHECK (track_count >= 0),
                added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT NOT NULL,
                duration_secs REAL NOT NULL CHECK (duration_secs >= 0),
                duration_str TEXT NOT NULL,
                folder_id INTEGER,
                is_favorite INTEGER DEFAULT 0,
                play_count INTEGER NOT NULL DEFAULT 0 CHECK (play_count >= 0),
                last_played_at TIMESTAMP,
                play_count_reset_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                file_modified INTEGER NOT NULL DEFAULT 0,
                lyrics_synced TEXT,
                lyrics_unsynced TEXT,
                rating INTEGER NOT NULL DEFAULT 0,
                track_number INTEGER DEFAULT 0,
                FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
            );
            CREATE TABLE IF NOT EXISTS cover_art (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                album_id INTEGER,
                track_id INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
                folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
                data BLOB NOT NULL,
                data_hash TEXT NOT NULL UNIQUE,
                width INTEGER CHECK (width > 0),
                height INTEGER CHECK (height > 0),
                mime_type TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position INTEGER NOT NULL,
                added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (playlist_id, track_id)
            );
            CREATE TABLE IF NOT EXISTS track_audio_features (
                track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                tempo REAL NOT NULL,
                rms_mean REAL NOT NULL,
                rms_std REAL NOT NULL,
                zcr_mean REAL NOT NULL,
                zcr_std REAL NOT NULL,
                spectral_centroid_mean REAL NOT NULL,
                spectral_centroid_std REAL NOT NULL,
                spectral_rolloff_mean REAL NOT NULL,
                spectral_rolloff_std REAL NOT NULL,
                spectral_flatness_mean REAL NOT NULL,
                spectral_flatness_std REAL NOT NULL,
                spectral_flux_mean REAL NOT NULL,
                spectral_flux_std REAL NOT NULL,
                hpr REAL NOT NULL DEFAULT 0.0,
                spectral_contrast_mean REAL NOT NULL DEFAULT 0.0,
                spectral_contrast_std REAL NOT NULL DEFAULT 0.0,
                crest_factor REAL NOT NULL DEFAULT 0.0,
                mode_major_ratio REAL NOT NULL DEFAULT 0.0,
                mfcc_json TEXT NOT NULL,
                chroma_json TEXT NOT NULL,
                analyzed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS track_mood_scores (
                track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
                happy REAL NOT NULL DEFAULT 0.0,
                sad REAL NOT NULL DEFAULT 0.0,
                calm REAL NOT NULL DEFAULT 0.0,
                energetic REAL NOT NULL DEFAULT 0.0,
                romantic REAL NOT NULL DEFAULT 0.0,
                party REAL NOT NULL DEFAULT 0.0,
                lofi REAL NOT NULL DEFAULT 0.0,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );",
        )?;

        // --- Indexes---------------------------------------------------
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_tracks_folder      ON tracks(folder_id);
             CREATE INDEX IF NOT EXISTS idx_tracks_album       ON tracks(album);
             CREATE INDEX IF NOT EXISTS idx_tracks_artist      ON tracks(artist);
             CREATE INDEX IF NOT EXISTS idx_tracks_favorite    ON tracks(is_favorite);
             CREATE INDEX IF NOT EXISTS idx_tracks_last_played ON tracks(last_played_at);
             CREATE INDEX IF NOT EXISTS idx_tracks_play_count  ON tracks(play_count);
             CREATE INDEX IF NOT EXISTS idx_tracks_play_count_reset ON tracks(play_count_reset_at);
             CREATE INDEX IF NOT EXISTS idx_cover_art_track ON cover_art(track_id);
             CREATE INDEX IF NOT EXISTS idx_cover_art_album ON cover_art(album_id);
             CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pid ON playlist_tracks(playlist_id);
             CREATE INDEX IF NOT EXISTS idx_playlist_tracks_tid ON playlist_tracks(track_id);
             CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pos
                 ON playlist_tracks(playlist_id, position);
             CREATE INDEX IF NOT EXISTS idx_mood_energetic ON track_mood_scores(energetic);
             CREATE INDEX IF NOT EXISTS idx_mood_calm ON track_mood_scores(calm);
             CREATE INDEX IF NOT EXISTS idx_mood_happy ON track_mood_scores(happy);
             CREATE INDEX IF NOT EXISTS idx_mood_sad ON track_mood_scores(sad);",
        )?;

        // --- Migrations-----------------------------------------------
        // Older app versions created the `tracks` table without the
        // `play_count_reset_at` column. CREATE TABLE IF NOT EXISTS does not
        // add the missing column on pre-existing databases, which made
        // run_maintenance_reset_if_needed fail with "no such column" and
        // prevented the app from starting. We probe the schema and ALTER
        // TABLE on demand.
        let has_reset_col = {
            let mut stmt = conn.prepare_cached("PRAGMA table_info(tracks)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found_reset = false;
            let mut found_track_num = false;
            for r in rows {
                let name = r?;
                if name == "play_count_reset_at" {
                    found_reset = true;
                }
                if name == "track_number" {
                    found_track_num = true;
                }
            }
            if !found_track_num {
                let _ = conn
                    .execute("ALTER TABLE tracks ADD COLUMN track_number INTEGER DEFAULT 0", []);
            }
            found_reset
        };
        if !has_reset_col {
            match conn.execute(
                "ALTER TABLE tracks ADD COLUMN play_count_reset_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
                [],
            ) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275 /* SQLITE_CONSTRAINT_COLUMN */
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) =>
                {
                }
                Err(e) => return Err(e.into()),
            }
        }

        let has_mtime_col = {
            let mut stmt = conn.prepare_cached("PRAGMA table_info(tracks)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for r in rows {
                if r? == "file_modified" {
                    found = true;
                }
            }
            found
        };
        if !has_mtime_col {
            match conn.execute(
                "ALTER TABLE tracks ADD COLUMN file_modified INTEGER NOT NULL DEFAULT 0",
                [],
            ) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
        }

        for col in &[
            "replaygain_track_db",
            "replaygain_album_db",
            "replaygain_track_peak",
            "replaygain_album_peak",
            "ebu_r128_loudness",
            "ebu_r128_peak",
        ] {
            let sql = format!("ALTER TABLE tracks ADD COLUMN {} REAL", col);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
        }

        for col in &["lyrics_synced", "lyrics_unsynced"] {
            let sql = format!("ALTER TABLE tracks ADD COLUMN {} TEXT", col);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
        }

        // rating column: 0-5 stars, 0 = unrated. CHECK constraint is enforced
        // at the application layer (the SQLite ALTER TABLE ADD COLUMN cannot
        // add a CHECK constraint; we validate in set_track_rating instead).
        {
            let sql = "ALTER TABLE tracks ADD COLUMN rating INTEGER NOT NULL DEFAULT 0";
            match conn.execute(sql, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
            // Now that the rating column definitely exists (either freshly
            // created in CREATE TABLE above, or just added by ALTER TABLE),
            // create the supporting index.
            conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_tracks_rating ON tracks(rating);")?;
        }

        for col in &[
            "hpr",
            "spectral_contrast_mean",
            "spectral_contrast_std",
            "crest_factor",
            "mode_major_ratio",
        ] {
            let sql =
                format!("ALTER TABLE track_audio_features ADD COLUMN {} REAL DEFAULT 0.0", col);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
        }

        {
            let sql = "ALTER TABLE track_mood_scores ADD COLUMN lofi REAL NOT NULL DEFAULT 0.0";
            match conn.execute(sql, []) {
                Ok(_) => {}
                Err(rusqlite::Error::SqliteFailure(err, msg))
                    if err.extended_code == 275
                        || msg.as_deref().is_some_and(|m| m.contains("duplicate column name")) => {}
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    // Settings
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT value FROM settings WHERE key = ?")?;
        let result = stmt.query_row(params![key], |row| row.get(0)).optional()?;
        Ok(result)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // Folders
    pub fn add_folder(&self, path: &str, name: &str, track_count: i32) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO folders (path, name, track_count) VALUES (?, ?, ?) ON CONFLICT(path) DO UPDATE SET name = excluded.name",
            params![path, name, track_count],
        )?;
        let mut stmt = conn.prepare_cached("SELECT id FROM folders WHERE path = ?")?;
        let id = stmt.query_row(params![path], |row| row.get(0))?;
        Ok(id)
    }

    pub fn get_all_folders(&self) -> Result<Vec<FolderRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT id, path, name, (SELECT COUNT(*) FROM tracks WHERE tracks.folder_id = folders.id), IFNULL(added_at, '') FROM folders ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                track_count: row.get(3)?,
                added_at: row.get(4)?,
            })
        })?;
        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn delete_folder(&self, id: i64) -> Result<(), DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM tracks WHERE folder_id = ?", params![id])?;
        tx.execute("DELETE FROM folders WHERE id = ?", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_track(&self, id: i64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM tracks WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Run a closure with a transaction. The closure receives a
    /// `&Transaction` and can execute multiple SQL statements; all are
    /// committed atomically on success, or rolled back on error.
    ///
    /// # Performance
    ///
    /// Wrapping a batch of inserts/updates in a single transaction is
    /// critical on HDD: without an explicit transaction, SQLite wraps
    /// every individual statement in its own implicit transaction, and
    /// each commit forces a write to the WAL file. For a 1000-track
    /// library scan, that's 1000 WAL writes vs. 1 — a 10-100x speedup
    /// on HDD where each write involves a seek.
    pub fn with_transaction<F, R>(&self, f: F) -> Result<R, DbError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> Result<R, DbError>,
    {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
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
            let placeholders =
                std::iter::repeat("?").take(chunk.len()).collect::<Vec<_>>().join(",");
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
                // Stale = path was tracked in DB but is NOT in the freshly-
                // scanned on-disk set. This avoids per-file stat() calls.
                if !on_disk.contains(path.as_str()) {
                    stale_ids.push(id);
                }
            }
        }
        let removed = stale_ids.len();
        for chunk in stale_ids.chunks(500) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders =
                std::iter::repeat("?").take(chunk.len()).collect::<Vec<_>>().join(",");
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

    /// Look up an album row id by `(album, album_artist)`. Returns `None`
    /// if no such album is referenced by any track. The scanner uses this
    /// to attach cover art to an album-level row.
    ///
    /// NOTE: there is no separate `albums` table; we treat `(album, artist)`
    /// as the album identity and return the min track id as a stand-in key.
    /// A future schema migration can add a real `albums` table.
    pub fn get_album_id(
        &self,
        album: &str,
        album_artist: Option<&str>,
    ) -> Result<Option<i64>, DbError> {
        // We pick the smallest track id that matches the album (and
        // album_artist, if provided). This is stable across scans.
        let conn = self.conn.lock();
        let row: Option<(i64,)> = match album_artist {
            Some(artist) => conn
                .query_row(
                    "SELECT MIN(id) FROM tracks WHERE album = ? AND artist = ?",
                    params![album, artist],
                    |r| Ok((r.get(0)?,)),
                )
                .optional()?,
            None => conn
                .query_row("SELECT MIN(id) FROM tracks WHERE album = ?", params![album], |r| {
                    Ok((r.get(0)?,))
                })
                .optional()?,
        };
        Ok(row.map(|(id,)| id))
    }

    /// Insert a cover-art row. Returns the new row id. The `cover_art`
    /// table is created by `init_schema`; the library scanner calls this
    /// when it has freshly extracted bytes from an audio file.
    pub fn insert_cover_art(
        &self,
        album_id: Option<i64>,
        track_id: Option<i64>,
        folder_id: Option<i64>,
        data: Option<&[u8]>,
        data_hash: Option<&str>,
        width: i32,
        height: i32,
        mime_type: &str,
    ) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO cover_art (album_id, track_id, folder_id, data, data_hash, width, height, mime_type)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(data_hash) DO NOTHING",
            params![album_id, track_id, folder_id, data, data_hash, width, height, mime_type],
        )?;
        let last_id = conn.last_insert_rowid();
        if last_id > 0 {
            // Fresh insert — return the new row id.
            return Ok(last_id);
        }
        // CONFLICT path: the row already exists. Look it up by data_hash
        // so the caller can associate the existing cover with the current
        // track (e.g., via a track_cover_art join or by updating track_id).
        if let Some(hash) = data_hash {
            let id: Option<i64> = conn
                .query_row("SELECT id FROM cover_art WHERE data_hash = ?", params![hash], |row| {
                    row.get(0)
                })
                .ok();
            if let Some(existing_id) = id {
                return Ok(existing_id);
            }
        }
        // Fallback: should not happen, but return 0 to indicate "no row
        // was inserted" rather than erroring.
        Ok(0)
    }

    /// Remove all cover art DB entries associated with a track id (used before updating cover image).
    pub fn delete_cover_art_by_track(&self, track_id: i64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM cover_art WHERE track_id = ?", params![track_id])?;
        Ok(())
    }

    // ========================================================================
    // Custom Playlists
    // ========================================================================

    /// Create a new playlist with the given name. Returns the new playlist id.
    pub fn create_playlist(&self, name: &str) -> Result<i64, DbError> {
        let conn = self.conn.lock();
        conn.execute("INSERT INTO playlists (name) VALUES (?)", params![name])?;
        Ok(conn.last_insert_rowid())
    }

    /// Rename an existing playlist. Returns `false` if no row was updated.
    pub fn rename_playlist(&self, playlist_id: i64, new_name: &str) -> Result<bool, DbError> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE playlists SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![new_name, playlist_id],
        )?;
        Ok(affected > 0)
    }

    /// Delete a playlist and all its track associations (cascades via FK).
    pub fn delete_playlist(&self, playlist_id: i64) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM playlists WHERE id = ?", params![playlist_id])?;
        Ok(())
    }

    /// Return all playlists ordered by name. `track_count` and `duration_secs`
    /// are computed via a single LEFT JOIN + GROUP BY, which SQLite executes
    /// in one pass (vs. two correlated subqueries per row in the old version).
    pub fn get_all_playlists(&self) -> Result<Vec<PlaylistRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT p.id, p.name, \
                    COUNT(pt.track_id) AS track_count, \
                    COALESCE(SUM(t.duration_secs), 0.0) AS total_secs, \
                    IFNULL(p.created_at, '') \
             FROM playlists p \
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id \
             LEFT JOIN tracks t ON t.id = pt.track_id \
             GROUP BY p.id \
             ORDER BY p.name ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlaylistRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                track_count: row.get(2)?,
                duration_secs: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Return a single playlist by id, or `None` if it does not exist.
    pub fn get_playlist(&self, playlist_id: i64) -> Result<Option<PlaylistRecord>, DbError> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT p.id, p.name,
                        (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id),
                        COALESCE((SELECT SUM(t.duration_secs) FROM playlist_tracks pt
                                  JOIN tracks t ON t.id = pt.track_id
                                  WHERE pt.playlist_id = p.id), 0),
                        IFNULL(p.created_at, '')
                 FROM playlists p
                 WHERE p.id = ?",
                params![playlist_id],
                |row| {
                    Ok(PlaylistRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        track_count: row.get(2)?,
                        duration_secs: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Append a track to the end of a playlist. If the track is already
    /// present, this is a no-op. Returns the new total track count.
    pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<i32, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        // Skip if already present (PRIMARY KEY (playlist_id, track_id) guards).
        let already: i64 = tx.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
            params![playlist_id, track_id],
            |r| r.get(0),
        )?;
        if already == 0 {
            let next_pos: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?",
                    params![playlist_id],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
                params![playlist_id, track_id, next_pos],
            )?;
            tx.execute(
                "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![playlist_id],
            )?;
        }
        let count: i32 = tx.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?",
            params![playlist_id],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(count)
    }

    /// Add many tracks to a playlist in a single transaction. Tracks that
    /// are already members are skipped. Returns the new total track count.
    pub fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_ids: &[i64],
    ) -> Result<i32, DbError> {
        if track_ids.is_empty() {
            return self.get_playlist(playlist_id).map(|p| p.map(|x| x.track_count).unwrap_or(0));
        }
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut next_pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?",
                params![playlist_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        for tid in track_ids {
            let already: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
                    params![playlist_id, tid],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if already > 0 {
                continue;
            }
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
                params![playlist_id, tid, next_pos],
            )?;
            next_pos += 1;
        }
        tx.execute(
            "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![playlist_id],
        )?;
        let count: i32 = tx.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?",
            params![playlist_id],
            |r| r.get(0),
        )?;
        tx.commit()?;
        Ok(count)
    }

    /// Remove a track from a playlist. Positions of the remaining tracks
    /// are compacted so they stay contiguous. Returns `true` if a row was
    /// removed.
    pub fn remove_track_from_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
    ) -> Result<bool, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let affected = tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
            params![playlist_id, track_id],
        )?;
        if affected > 0 {
            // Compact positions by reading remaining track_ids in order and
            // renumbering them 0, 1, 2, ... one at a time. SQLite's
            // `UPDATE ... SET position = (SELECT COUNT(*) FROM same_table)`
            // is non-deterministic across rows because the subquery reads
            // mid-update state; doing it from Rust is safer.
            let mut stmt = tx.prepare(
                "SELECT track_id FROM playlist_tracks
                 WHERE playlist_id = ?
                 ORDER BY position ASC",
            )?;
            let ids: Vec<i64> = stmt
                .query_map(params![playlist_id], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            drop(stmt);
            for (new_pos, tid) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE playlist_tracks SET position = ?
                     WHERE playlist_id = ? AND track_id = ?",
                    params![new_pos as i64, playlist_id, tid],
                )?;
            }
            tx.execute(
                "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                params![playlist_id],
            )?;
        }
        tx.commit()?;
        Ok(affected > 0)
    }

    /// Return all tracks in a playlist, in playlist order.
    pub fn get_tracks_by_playlist(&self, playlist_id: i64) -> Result<Vec<TrackRecord>, DbError> {
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks t
                 JOIN playlist_tracks pt ON pt.track_id = t.id
                 WHERE pt.playlist_id = ?
                 ORDER BY pt.position ASC",
                Self::TRACK_SELECT_COLS
            ),
            params![playlist_id],
        )
    }

    /// Reorder a track within a playlist by moving it to a new position.
    /// `new_position` is 0-indexed; values outside the valid range are
    /// clamped. Returns `true` if the order changed.
    pub fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        new_position: i32,
    ) -> Result<bool, DbError> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let current: Option<i64> = tx
            .query_row(
                "SELECT position FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
                params![playlist_id, track_id],
                |r| r.get(0),
            )
            .ok();
        let Some(current_pos) = current else {
            tx.commit()?;
            return Ok(false);
        };
        let max_pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE playlist_id = ?",
                params![playlist_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let new_pos = new_position.max(0) as i64;
        let new_pos = new_pos.min(max_pos);
        if new_pos == current_pos {
            tx.commit()?;
            return Ok(false);
        }
        // Shift positions in the affected range.
        if new_pos < current_pos {
            tx.execute(
                "UPDATE playlist_tracks SET position = position + 1
                 WHERE playlist_id = ? AND position >= ? AND position < ?",
                params![playlist_id, new_pos, current_pos],
            )?;
        } else {
            tx.execute(
                "UPDATE playlist_tracks SET position = position - 1
                 WHERE playlist_id = ? AND position > ? AND position <= ?",
                params![playlist_id, current_pos, new_pos],
            )?;
        }
        tx.execute(
            "UPDATE playlist_tracks SET position = ?
             WHERE playlist_id = ? AND track_id = ?",
            params![new_pos, playlist_id, track_id],
        )?;
        tx.execute(
            "UPDATE playlists SET updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            params![playlist_id],
        )?;
        tx.commit()?;
        Ok(true)
    }

    // ========================================================================
    // Ratings
    // ========================================================================

    /// Set the user rating of a track. `rating` is clamped to -1..=5
    /// (0 = unrated, -1 = disliked). Returns the clamped value stored.
    pub fn set_track_rating(&self, track_id: i64, rating: i32) -> Result<i32, DbError> {
        let clamped = rating.clamp(-1, 5);
        let conn = self.conn.lock();
        conn.execute("UPDATE tracks SET rating = ? WHERE id = ?", params![clamped, track_id])?;
        Ok(clamped)
    }

    /// Return all tracks with a specific rating (1..=5). Pass `0` to fetch
    /// unrated tracks. Used by autoplaylists.
    pub fn get_tracks_by_rating(
        &self,
        rating: i32,
        limit: usize,
    ) -> Result<Vec<TrackRecord>, DbError> {
        let clamped = rating.clamp(0, 5);
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE rating = ? ORDER BY play_count DESC, title ASC LIMIT ?",
                Self::TRACK_SELECT_COLS
            ),
            params![clamped, limit as i64],
        )
    }

    /// Return all tracks with rating >= `min_rating`. Used by autoplaylists
    /// like "4+ stars".
    pub fn get_tracks_with_min_rating(
        &self,
        min_rating: i32,
        limit: usize,
    ) -> Result<Vec<TrackRecord>, DbError> {
        let clamped = min_rating.clamp(1, 5);
        self.query_tracks(
            &format!(
                "SELECT {} FROM tracks WHERE rating >= ? ORDER BY rating DESC, play_count DESC, title ASC LIMIT ?",
                Self::TRACK_SELECT_COLS
            ),
            params![clamped, limit as i64],
        )
    }

    // ========================================================================
    // Albums & Artists Aggregators
    // ========================================================================

    /// Return one row per distinct `(album, artist)` tuple, with track count,
    /// total duration, and the year (taken from the most recent track's
    /// file_modified as a fallback; real year lives on `models::Track`, not
    /// in the SQL schema, so we approximate). Ordered alphabetically.
    pub fn get_all_albums(&self) -> Result<Vec<AlbumRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT MIN(t.id) AS id, t.album AS album, t.artist AS album_artist,
                    COUNT(*) AS track_count, COALESCE(SUM(t.duration_secs), 0) AS duration_secs,
                    NULL AS year
             FROM tracks t
             WHERE t.album != ''
             GROUP BY t.album, t.artist
             ORDER BY t.album COLLATE NOCASE ASC, t.artist COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(AlbumRecord {
                id: row.get(0)?,
                album: row.get(1)?,
                album_artist: row.get(2)?,
                track_count: row.get(3)?,
                duration_secs: row.get(4)?,
                year: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Return one row per distinct artist, with album count and track count.
    pub fn get_all_artists(&self) -> Result<Vec<ArtistRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT MIN(t.id) AS id, t.artist AS artist,
                    COUNT(DISTINCT t.album) AS album_count,
                    COUNT(*) AS track_count
             FROM tracks t
             WHERE t.artist != ''
             GROUP BY t.artist
             ORDER BY t.artist COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ArtistRecord {
                id: row.get(0)?,
                artist: row.get(1)?,
                album_count: row.get(2)?,
                track_count: row.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Return all albums by a given artist.
    pub fn get_albums_by_artist(&self, artist: &str) -> Result<Vec<AlbumRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT MIN(t.id) AS id, t.album AS album, t.artist AS album_artist,
                    COUNT(*) AS track_count, COALESCE(SUM(t.duration_secs), 0) AS duration_secs,
                    NULL AS year
             FROM tracks t
             WHERE t.artist = ? AND t.album != ''
             GROUP BY t.album, t.artist
             ORDER BY t.album COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map(params![artist], |row| {
            Ok(AlbumRecord {
                id: row.get(0)?,
                album: row.get(1)?,
                album_artist: row.get(2)?,
                track_count: row.get(3)?,
                duration_secs: row.get(4)?,
                year: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn run_maintenance_reset_if_needed(&self) -> Result<(), DbError> {
        let conn = self.conn.lock();
        // 30-day reset cycle: reset play stats only for tracks not played in 30 days.
        conn.execute(
            "UPDATE tracks SET play_count = 0, last_played_at = NULL, play_count_reset_at = CURRENT_TIMESTAMP
             WHERE (strftime('%s', 'now') - strftime('%s', play_count_reset_at)) > 2592000",
            params![],
        )?;
        Ok(())
    }

    pub const TRACK_SELECT_COLS: &'static str = "id, path, title, artist, album, duration_secs, duration_str, folder_id, is_favorite, play_count, IFNULL(last_played_at, ''), file_modified, replaygain_track_db, replaygain_album_db, replaygain_track_peak, replaygain_album_peak, ebu_r128_loudness, ebu_r128_peak, lyrics_synced, lyrics_unsynced, rating, track_number";

    fn query_tracks<P: rusqlite::Params>(
        &self,
        sql: &str,
        params: P,
    ) -> Result<Vec<TrackRecord>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt.query_map(params, |row| {
            let fav_int: i32 = row.get(8)?;
            let last_played: String = row.get(10)?;
            let file_modified: i64 = row.get(11).unwrap_or(0);
            let rating: i32 = row.get(20).unwrap_or(0);
            Ok(TrackRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                artist: row.get(3)?,
                album: row.get(4)?,
                duration_secs: row.get(5)?,
                duration_str: row.get(6)?,
                folder_id: row.get(7)?,
                is_favorite: fav_int != 0,
                play_count: row.get(9)?,
                last_played_at: if last_played.is_empty() { None } else { Some(last_played) },
                file_modified,
                replaygain_track_db: row.get(12).unwrap_or(None),
                replaygain_album_db: row.get(13).unwrap_or(None),
                replaygain_track_peak: row.get(14).unwrap_or(None),
                replaygain_album_peak: row.get(15).unwrap_or(None),
                ebu_r128_loudness: row.get(16).unwrap_or(None),
                ebu_r128_peak: row.get(17).unwrap_or(None),
                lyrics_synced: row.get(18).unwrap_or(None),
                lyrics_unsynced: row.get(19).unwrap_or(None),
                rating,
                track_number: row.get(21).unwrap_or(None),
            })
        })?;
        let mut tracks = Vec::new();
        for t in rows {
            tracks.push(t?);
        }
        Ok(tracks)
    }

    // --- Audio Features & Mood Scores -----------------------------------------
    pub fn save_audio_features(
        &self,
        features: &crate::models::TrackAudioFeatures,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO track_audio_features (
                track_id, tempo, rms_mean, rms_std, zcr_mean, zcr_std,
                spectral_centroid_mean, spectral_centroid_std,
                spectral_rolloff_mean, spectral_rolloff_std,
                spectral_flatness_mean, spectral_flatness_std,
                spectral_flux_mean, spectral_flux_std,
                hpr, spectral_contrast_mean, spectral_contrast_std, crest_factor, mode_major_ratio,
                mfcc_json, chroma_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(track_id) DO UPDATE SET
                tempo = excluded.tempo,
                rms_mean = excluded.rms_mean,
                rms_std = excluded.rms_std,
                zcr_mean = excluded.zcr_mean,
                zcr_std = excluded.zcr_std,
                spectral_centroid_mean = excluded.spectral_centroid_mean,
                spectral_centroid_std = excluded.spectral_centroid_std,
                spectral_rolloff_mean = excluded.spectral_rolloff_mean,
                spectral_rolloff_std = excluded.spectral_rolloff_std,
                spectral_flatness_mean = excluded.spectral_flatness_mean,
                spectral_flatness_std = excluded.spectral_flatness_std,
                spectral_flux_mean = excluded.spectral_flux_mean,
                spectral_flux_std = excluded.spectral_flux_std,
                hpr = excluded.hpr,
                spectral_contrast_mean = excluded.spectral_contrast_mean,
                spectral_contrast_std = excluded.spectral_contrast_std,
                crest_factor = excluded.crest_factor,
                mode_major_ratio = excluded.mode_major_ratio,
                mfcc_json = excluded.mfcc_json,
                chroma_json = excluded.chroma_json,
                analyzed_at = CURRENT_TIMESTAMP",
            params![
                features.track_id,
                features.tempo as f64,
                features.rms_mean as f64,
                features.rms_std as f64,
                features.zcr_mean as f64,
                features.zcr_std as f64,
                features.spectral_centroid_mean as f64,
                features.spectral_centroid_std as f64,
                features.spectral_rolloff_mean as f64,
                features.spectral_rolloff_std as f64,
                features.spectral_flatness_mean as f64,
                features.spectral_flatness_std as f64,
                features.spectral_flux_mean as f64,
                features.spectral_flux_std as f64,
                features.hpr as f64,
                features.spectral_contrast_mean as f64,
                features.spectral_contrast_std as f64,
                features.crest_factor as f64,
                features.mode_major_ratio as f64,
                features.mfcc_json,
                features.chroma_json,
            ],
        )?;
        Ok(())
    }

    pub fn get_audio_features(
        &self,
        track_id: i64,
    ) -> Result<Option<crate::models::TrackAudioFeatures>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT track_id, tempo, rms_mean, rms_std, zcr_mean, zcr_std,
                    spectral_centroid_mean, spectral_centroid_std,
                    spectral_rolloff_mean, spectral_rolloff_std,
                    spectral_flatness_mean, spectral_flatness_std,
                    spectral_flux_mean, spectral_flux_std,
                    hpr, spectral_contrast_mean, spectral_contrast_std, crest_factor, mode_major_ratio,
                    mfcc_json, chroma_json
             FROM track_audio_features WHERE track_id = ?",
        )?;
        let res = stmt
            .query_row(params![track_id], |row| {
                Ok(crate::models::TrackAudioFeatures {
                    track_id: row.get(0)?,
                    tempo: row.get::<_, f64>(1)? as f32,
                    rms_mean: row.get::<_, f64>(2)? as f32,
                    rms_std: row.get::<_, f64>(3)? as f32,
                    zcr_mean: row.get::<_, f64>(4)? as f32,
                    zcr_std: row.get::<_, f64>(5)? as f32,
                    spectral_centroid_mean: row.get::<_, f64>(6)? as f32,
                    spectral_centroid_std: row.get::<_, f64>(7)? as f32,
                    spectral_rolloff_mean: row.get::<_, f64>(8)? as f32,
                    spectral_rolloff_std: row.get::<_, f64>(9)? as f32,
                    spectral_flatness_mean: row.get::<_, f64>(10)? as f32,
                    spectral_flatness_std: row.get::<_, f64>(11)? as f32,
                    spectral_flux_mean: row.get::<_, f64>(12)? as f32,
                    spectral_flux_std: row.get::<_, f64>(13)? as f32,
                    hpr: row.get::<_, f64>(14)? as f32,
                    spectral_contrast_mean: row.get::<_, f64>(15)? as f32,
                    spectral_contrast_std: row.get::<_, f64>(16)? as f32,
                    crest_factor: row.get::<_, f64>(17)? as f32,
                    mode_major_ratio: row.get::<_, f64>(18)? as f32,
                    mfcc_json: row.get(19)?,
                    chroma_json: row.get(20)?,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub fn save_mood_scores(&self, scores: &crate::models::TrackMoodScores) -> Result<(), DbError> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO track_mood_scores (
                track_id, happy, sad, calm, energetic, romantic, party, lofi
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(track_id) DO UPDATE SET
                happy = excluded.happy,
                sad = excluded.sad,
                calm = excluded.calm,
                energetic = excluded.energetic,
                romantic = excluded.romantic,
                party = excluded.party,
                lofi = excluded.lofi,
                updated_at = CURRENT_TIMESTAMP",
            params![
                scores.track_id,
                scores.happy as f64,
                scores.sad as f64,
                scores.calm as f64,
                scores.energetic as f64,
                scores.romantic as f64,
                scores.party as f64,
                scores.lofi as f64,
            ],
        )?;
        Ok(())
    }

    pub fn get_mood_scores(
        &self,
        track_id: i64,
    ) -> Result<Option<crate::models::TrackMoodScores>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT track_id, happy, sad, calm, energetic, romantic, party, lofi
             FROM track_mood_scores WHERE track_id = ?",
        )?;
        let res = stmt
            .query_row(params![track_id], |row| {
                Ok(crate::models::TrackMoodScores {
                    track_id: row.get(0)?,
                    happy: row.get::<_, f64>(1)? as f32,
                    sad: row.get::<_, f64>(2)? as f32,
                    calm: row.get::<_, f64>(3)? as f32,
                    energetic: row.get::<_, f64>(4)? as f32,
                    romantic: row.get::<_, f64>(5)? as f32,
                    party: row.get::<_, f64>(6)? as f32,
                    lofi: row.get::<_, f64>(7)? as f32,
                })
            })
            .optional()?;
        Ok(res)
    }

    pub const DEFAULT_MOOD_CONFIDENCE_THRESHOLD: f32 = 0.70;

    pub fn get_tracks_by_mood(
        &self,
        mood: &str,
        min_score: f32,
    ) -> Result<Vec<TrackRecord>, DbError> {
        let col = match mood.to_lowercase().as_str() {
            "happy" => "happy",
            "sad" => "sad",
            "calm" => "calm",
            "energetic" => "energetic",
            "romantic" => "romantic",
            "party" => "party",
            "lofi" => "lofi",
            _ => return Err(DbError::Other(format!("Invalid mood name: {}", mood))),
        };
        let sql = format!(
            "SELECT {} FROM tracks JOIN track_mood_scores ON tracks.id = track_mood_scores.track_id WHERE track_mood_scores.{} >= ? ORDER BY tracks.title ASC",
            Self::TRACK_SELECT_COLS,
            col
        );
        self.query_tracks(&sql, params![min_score as f64])
    }

    /// Retrieve tracks matching a mood with the default high-confidence threshold (0.70).
    pub fn get_tracks_by_mood_default(&self, mood: &str) -> Result<Vec<TrackRecord>, DbError> {
        self.get_tracks_by_mood(mood, Self::DEFAULT_MOOD_CONFIDENCE_THRESHOLD)
    }

    pub fn get_all_mood_scores(&self) -> Result<Vec<crate::models::TrackMoodScores>, DbError> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT track_id, happy, sad, calm, energetic, romantic, party, lofi
             FROM track_mood_scores",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::models::TrackMoodScores {
                track_id: row.get(0)?,
                happy: row.get::<_, f64>(1)? as f32,
                sad: row.get::<_, f64>(2)? as f32,
                calm: row.get::<_, f64>(3)? as f32,
                energetic: row.get::<_, f64>(4)? as f32,
                romantic: row.get::<_, f64>(5)? as f32,
                party: row.get::<_, f64>(6)? as f32,
                lofi: row.get::<_, f64>(7)? as f32,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_top_moods_batch(
        &self,
        min_score: f32,
    ) -> Result<std::collections::HashMap<i64, String>, DbError> {
        let scores = self.get_all_mood_scores()?;
        let mut map = std::collections::HashMap::with_capacity(scores.len());
        for s in scores {
            if let Some((mood_name, _score)) = s.top_mood(min_score) {
                map.insert(s.track_id, mood_name);
            }
        }
        let fallback_moods = ["Energetic", "Happy", "Calm", "Romantic", "Party", "Lofi", "Sad"];
        let conn = self.conn.lock();
        if let Ok(mut stmt) = conn.prepare_cached("SELECT id FROM tracks") {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, i64>(0)) {
                for tid in rows.flatten() {
                    map.entry(tid).or_insert_with(|| {
                        let idx = (tid.unsigned_abs() as usize) % fallback_moods.len();
                        fallback_moods[idx].to_string()
                    });
                }
            }
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_init_and_crud() {
        let db = PlayTuneDb::open_in_memory().unwrap();
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("dark".to_string()));

        let folder_id = db.add_folder("/mnt/music/pop", "Pop Music", 10).unwrap();
        let folders = db.get_all_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Pop Music");

        let track_id = db
            .add_or_update_track(
                "/mnt/music/pop/song1.mp3",
                "Song One",
                "Artist One",
                "Album One",
                210.5,
                "3:30",
                Some(folder_id),
                0,
            )
            .unwrap();

        let tracks = db.get_all_tracks().unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, track_id);
        assert_eq!(tracks[0].title, "Song One");

        let is_fav = db.toggle_favorite(track_id).unwrap();
        assert!(is_fav);
        assert_eq!(db.get_favorite_tracks().unwrap().len(), 1);

        db.record_play(track_id).unwrap();
        db.record_play(track_id).unwrap();
        db.record_play(track_id).unwrap();
        let most_played = db.get_most_played_tracks(10).unwrap();
        assert_eq!(most_played.len(), 1);
        assert_eq!(most_played[0].play_count, 3);
    }

    #[test]
    fn test_playlists_crud() {
        let db = PlayTuneDb::open_in_memory().unwrap();

        let folder_id = db.add_folder("/mnt/music/pop", "Pop Music", 0).unwrap();
        let t1 = db
            .add_or_update_track(
                "/m/a/1.mp3",
                "Song One",
                "Artist A",
                "Album X",
                200.0,
                "3:20",
                Some(folder_id),
                0,
            )
            .unwrap();
        let t2 = db
            .add_or_update_track(
                "/m/a/2.mp3",
                "Song Two",
                "Artist A",
                "Album X",
                180.0,
                "3:00",
                Some(folder_id),
                0,
            )
            .unwrap();
        let t3 = db
            .add_or_update_track(
                "/m/a/3.mp3",
                "Song Three",
                "Artist B",
                "Album Y",
                240.0,
                "4:00",
                Some(folder_id),
                0,
            )
            .unwrap();

        // Create playlist
        let pid = db.create_playlist("Road Trip").unwrap();
        assert!(pid > 0);

        // Add tracks
        let n = db.add_track_to_playlist(pid, t1).unwrap();
        assert_eq!(n, 1);
        let n = db.add_tracks_to_playlist(pid, &[t2, t3]).unwrap();
        assert_eq!(n, 3);

        // Duplicate insert is a no-op
        let n = db.add_track_to_playlist(pid, t1).unwrap();
        assert_eq!(n, 3);

        // List tracks
        let tracks = db.get_tracks_by_playlist(pid).unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].title, "Song One");
        assert_eq!(tracks[1].title, "Song Two");
        assert_eq!(tracks[2].title, "Song Three");

        // Move track 0 to position 2
        let moved = db.move_track_in_playlist(pid, t1, 2).unwrap();
        assert!(moved);
        let tracks = db.get_tracks_by_playlist(pid).unwrap();
        assert_eq!(tracks[0].title, "Song Two");
        assert_eq!(tracks[1].title, "Song Three");
        assert_eq!(tracks[2].title, "Song One");

        // Remove a track — positions compact
        db.remove_track_from_playlist(pid, t2).unwrap();
        let tracks = db.get_tracks_by_playlist(pid).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Song Three");
        assert_eq!(tracks[1].title, "Song One");

        // Rename playlist
        let ok = db.rename_playlist(pid, "Night Drive").unwrap();
        assert!(ok);
        let pl = db.get_playlist(pid).unwrap().unwrap();
        assert_eq!(pl.name, "Night Drive");
        assert_eq!(pl.track_count, 2);

        // List all playlists
        let all = db.get_all_playlists().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Night Drive");

        // Delete playlist cascades
        db.delete_playlist(pid).unwrap();
        assert!(db.get_playlist(pid).unwrap().is_none());
        let all = db.get_all_playlists().unwrap();
        assert_eq!(all.len(), 0);
    }

    #[test]
    fn test_ratings() {
        let db = PlayTuneDb::open_in_memory().unwrap();
        let t1 = db
            .add_or_update_track(
                "/m/a/1.mp3",
                "Song One",
                "Artist A",
                "Album X",
                200.0,
                "3:20",
                None,
                0,
            )
            .unwrap();
        let t2 = db
            .add_or_update_track(
                "/m/a/2.mp3",
                "Song Two",
                "Artist A",
                "Album X",
                180.0,
                "3:00",
                None,
                0,
            )
            .unwrap();

        // Initial rating is 0
        let tracks = db.get_all_tracks().unwrap();
        assert_eq!(tracks[0].rating, 0);

        // Set rating
        let stored = db.set_track_rating(t1, 4).unwrap();
        assert_eq!(stored, 4);
        let stored = db.set_track_rating(t2, 5).unwrap();
        assert_eq!(stored, 5);

        // Clamping
        let stored = db.set_track_rating(t1, 99).unwrap();
        assert_eq!(stored, 5);
        let stored = db.set_track_rating(t1, -3).unwrap();
        assert_eq!(stored, -1);

        // Query by rating
        let fives = db.get_tracks_by_rating(5, 100).unwrap();
        assert_eq!(fives.len(), 1);
        assert_eq!(fives[0].title, "Song Two");

        // Min rating query
        let four_plus = db.get_tracks_with_min_rating(4, 100).unwrap();
        assert_eq!(four_plus.len(), 1);
    }

    #[test]
    fn test_albums_and_artists_aggregators() {
        let db = PlayTuneDb::open_in_memory().unwrap();
        db.add_or_update_track(
            "/m/a/1.mp3",
            "Song One",
            "Artist A",
            "Album X",
            200.0,
            "3:20",
            None,
            0,
        )
        .unwrap();
        db.add_or_update_track(
            "/m/a/2.mp3",
            "Song Two",
            "Artist A",
            "Album X",
            180.0,
            "3:00",
            None,
            0,
        )
        .unwrap();
        db.add_or_update_track(
            "/m/a/3.mp3",
            "Song Three",
            "Artist A",
            "Album Y",
            240.0,
            "4:00",
            None,
            0,
        )
        .unwrap();
        db.add_or_update_track(
            "/m/a/4.mp3",
            "Song Four",
            "Artist B",
            "Album Z",
            220.0,
            "3:40",
            None,
            0,
        )
        .unwrap();

        let albums = db.get_all_albums().unwrap();
        assert_eq!(albums.len(), 3); // X, Y, Z

        let artists = db.get_all_artists().unwrap();
        assert_eq!(artists.len(), 2); // A and B
        let a = artists.iter().find(|a| a.artist == "Artist A").unwrap();
        assert_eq!(a.album_count, 2);
        assert_eq!(a.track_count, 3);

        let a_albums = db.get_albums_by_artist("Artist A").unwrap();
        assert_eq!(a_albums.len(), 2);
    }

    #[test]
    fn test_audio_features_and_mood_scores() {
        let db = PlayTuneDb::open_in_memory().unwrap();
        let track_id = db
            .add_or_update_track(
                "/music/energetic_song.mp3",
                "Upbeat Track",
                "Artist X",
                "Album Y",
                210.0,
                "3:30",
                None,
                0,
            )
            .unwrap();

        let features = crate::models::TrackAudioFeatures {
            track_id,
            tempo: 128.0,
            rms_mean: 0.45,
            rms_std: 0.05,
            zcr_mean: 0.12,
            zcr_std: 0.02,
            spectral_centroid_mean: 2500.0,
            spectral_centroid_std: 300.0,
            spectral_rolloff_mean: 5000.0,
            spectral_rolloff_std: 400.0,
            spectral_flatness_mean: 0.01,
            spectral_flatness_std: 0.002,
            spectral_flux_mean: 0.8,
            spectral_flux_std: 0.1,
            mfcc_json: "[0.1,0.2]".to_string(),
            chroma_json: "[0.5,0.5]".to_string(),
        };

        db.save_audio_features(&features).unwrap();
        let loaded_features = db.get_audio_features(track_id).unwrap().unwrap();
        assert_eq!(loaded_features.tempo, 128.0);
        assert_eq!(loaded_features.rms_mean, 0.45);

        let mood_scores = crate::models::TrackMoodScores {
            track_id,
            happy: 0.85,
            sad: 0.02,
            calm: 0.10,
            energetic: 0.92,
            romantic: 0.05,
            party: 0.88,
            nostalgic: 0.15,
            sleep: 0.01,
        };

        db.save_mood_scores(&mood_scores).unwrap();
        let loaded_scores = db.get_mood_scores(track_id).unwrap().unwrap();
        assert_eq!(loaded_scores.energetic, 0.92);
        assert_eq!(loaded_scores.happy, 0.85);

        let energetic_tracks = db.get_tracks_by_mood("energetic", 0.70).unwrap();
        assert_eq!(energetic_tracks.len(), 1);
        assert_eq!(energetic_tracks[0].title, "Upbeat Track");

        let sad_tracks = db.get_tracks_by_mood("sad", 0.70).unwrap();
        assert_eq!(sad_tracks.len(), 0);
    }
}

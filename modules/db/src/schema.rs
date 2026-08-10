use rusqlite::Connection;

use crate::database::{DbError, PlayTuneDb};

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
}

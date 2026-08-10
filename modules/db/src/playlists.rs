use rusqlite::{params, OptionalExtension};

use crate::database::{DbError, PlayTuneDb, PlaylistRecord, TrackRecord};

impl PlayTuneDb {
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
}

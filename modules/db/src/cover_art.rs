use rusqlite::{params, OptionalExtension};

use crate::database::{DbError, PlayTuneDb};

impl PlayTuneDb {
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
        let rows_affected = conn.execute(
            "INSERT INTO cover_art (album_id, track_id, folder_id, data, data_hash, width, height, mime_type)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(data_hash) DO NOTHING",
            params![album_id, track_id, folder_id, data, data_hash, width, height, mime_type],
        )?;
        if rows_affected > 0 {
            // Fresh insert — return the new row id.
            return Ok(conn.last_insert_rowid());
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
}

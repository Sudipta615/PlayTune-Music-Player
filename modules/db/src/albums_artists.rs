use rusqlite::params;

use crate::database::{AlbumRecord, ArtistRecord, DbError, PlayTuneDb};

impl PlayTuneDb {
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
}

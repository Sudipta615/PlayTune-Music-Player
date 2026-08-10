use rusqlite::params;

use crate::database::{DbError, PlayTuneDb, TrackRecord};

impl PlayTuneDb {
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
}

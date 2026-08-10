use rusqlite::{params, OptionalExtension};

use crate::database::{DbError, PlayTuneDb, TrackRecord};

impl PlayTuneDb {
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

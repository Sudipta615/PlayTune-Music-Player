//! Data models for the PlayTune database.
//!
//! `TrackRecord` (in `database.rs`) is the lightweight row representation
//! returned by SQL queries. `Track` (here) is the richer domain object used
//! by the library scanner, which collects every metadata field that symphonia
//! exposes — file size, bitrate, codec format, ReplayGain tags, etc. The
//! scanner writes a subset of these columns to the DB and keeps the rest
//! in memory for the running session.

use chrono::NaiveDateTime;

/// Full track metadata extracted during a library scan.
#[derive(Debug, Clone, Default)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    /// Duration in seconds. `f32` is sufficient for human-scale track
    /// lengths (sub-millisecond precision at 1 hour).
    pub duration_secs: f32,
    /// Pre-formatted "M:SS" / "H:MM:SS" string for display.
    pub duration_str: String,
    pub sample_rate: i32,
    pub channels: i32,
    pub bitrate_kbps: Option<i32>,
    /// Lowercased codec/container extension ("mp3", "flac", ...).
    pub format: String,
    pub file_size: i64,
    pub file_modified: i64,
    pub crc32: Option<u32>,
    pub replaygain_track_db: Option<f32>,
    pub replaygain_album_db: Option<f32>,
    pub replaygain_track_peak: Option<f32>,
    pub replaygain_album_peak: Option<f32>,
    pub ebu_r128_loudness: Option<f32>,
    pub ebu_r128_peak: Option<f32>,
    pub bpm: Option<f32>,
    pub lyrics_synced: Option<String>,
    pub lyrics_unsynced: Option<String>,
    /// 0–5 star user rating (0 = unrated).
    pub rating: i32,
    pub last_played: Option<NaiveDateTime>,
    pub play_count: i32,
    pub date_added: NaiveDateTime,
    pub date_scanned: NaiveDateTime,
    /// Folder row id this track belongs to (foreign key into `folders`).
    pub folder_id: Option<i64>,
}

impl Track {
    /// Build a default "unknown" track used as a placeholder.
    pub fn empty(path: impl Into<String>) -> Self {
        let now = chrono::Utc::now().naive_utc();
        Self {
            id: 0,
            path: path.into(),
            title: "Unknown".to_string(),
            duration_str: "0:00".to_string(),
            date_added: now,
            date_scanned: now,
            ..Default::default()
        }
    }
}

/// Album-level cover-art row (placeholder; the schema is owned by the
/// library crate's optional `cover_art` table). Kept here so callers can
/// reference `db::models::AlbumCover` without a hard dependency on the
/// library crate.
#[derive(Debug, Clone, Default)]
pub struct AlbumCover {
    pub id: i64,
    pub album_id: Option<i64>,
    pub track_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub data: Vec<u8>,
    pub data_hash: String,
    pub width: i32,
    pub height: i32,
    pub mime_type: String,
}

/// Extracted acoustic DSP features cached in the database per song.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackAudioFeatures {
    pub track_id: i64,
    pub tempo: f32,
    pub rms_mean: f32,
    pub rms_std: f32,
    pub zcr_mean: f32,
    pub zcr_std: f32,
    pub spectral_centroid_mean: f32,
    pub spectral_centroid_std: f32,
    pub spectral_rolloff_mean: f32,
    pub spectral_rolloff_std: f32,
    pub spectral_flatness_mean: f32,
    pub spectral_flatness_std: f32,
    pub spectral_flux_mean: f32,
    pub spectral_flux_std: f32,
    pub hpr: f32,
    pub spectral_contrast_mean: f32,
    pub spectral_contrast_std: f32,
    pub crest_factor: f32,
    pub mode_major_ratio: f32,
    /// 13 MFCC pairs [mean, std] serialized as JSON
    pub mfcc_json: String,
    /// 12 Chroma pitch class magnitudes serialized as JSON
    pub chroma_json: String,
}

/// Predicted mood probabilities (0.0 to 1.0) cached per song.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TrackMoodScores {
    pub track_id: i64,
    pub happy: f32,
    pub sad: f32,
    pub calm: f32,
    pub energetic: f32,
    pub romantic: f32,
    pub party: f32,
    pub lofi: f32,
}

impl TrackMoodScores {
    /// Return the highest-scoring mood name and its score if it exceeds `min_score`.
    /// Falls back to argmax if all scores are below `min_score`.
    pub fn top_mood(&self, min_score: f32) -> Option<(String, f32)> {
        let moods = [
            ("Energetic", self.energetic),
            ("Happy", self.happy),
            ("Party", self.party),
            ("Calm", self.calm),
            ("Romantic", self.romantic),
            ("Sad", self.sad),
            ("Lofi", self.lofi),
        ];

        let mut best: Option<(&'static str, f32)> = None;
        for (name, score) in moods {
            if score >= min_score {
                match best {
                    Some((_, best_score)) => {
                        if score > best_score {
                            best = Some((name, score));
                        }
                    }
                    None => {
                        best = Some((name, score));
                    }
                }
            }
        }

        best.or_else(|| {
            moods
                .into_iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        })
        .map(|(name, score)| (name.to_string(), score))
    }
}

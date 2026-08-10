use rusqlite::Connection;
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
    /// Stable identifier (smallest track id by the artist)
    pub id: i64,
    pub artist: String,
    pub album_count: i32,
    pub track_count: i32,
}

/// The main database handle. All sub-modules add `impl PlayTuneDb` blocks
/// that extend this type with their domain-specific methods.
pub struct PlayTuneDb {
    pub(crate) conn: parking_lot::Mutex<Connection>,
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
            ..Default::default()
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
            lofi: 0.01,
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

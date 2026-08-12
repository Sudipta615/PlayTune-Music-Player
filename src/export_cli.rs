use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use analysis::AudioFeatureExtractor;
use db::PlayTuneDb;

pub fn export_training_data(db: &PlayTuneDb, output_csv_path: &str) -> Result<(), String> {
    log::info!("Starting training data export to {}", output_csv_path);
    let playlists = db.get_all_playlists().map_err(|e| e.to_string())?;

    let target_moods = ["happy", "sad", "calm", "energetic", "romantic", "party", "lofi"];

    // Find playlists starting with "Mood - " (case insensitive)
    let mut mood_playlists: HashMap<String, i64> = HashMap::new();
    for pl in &playlists {
        let name_lower = pl.name.to_lowercase();
        if name_lower.starts_with("mood - ") || name_lower.starts_with("mood-") {
            let mood_part =
                name_lower.trim_start_matches("mood - ").trim_start_matches("mood-").trim();
            if target_moods.contains(&mood_part) {
                mood_playlists.insert(mood_part.to_string(), pl.id);
            }
        }
    }

    if mood_playlists.is_empty() {
        return Err(
            "No playlists starting with 'Mood - <Name>' were found (e.g., 'Mood - Happy', 'Mood - Energetic').".to_string(),
        );
    }

    log::info!("Found {} mood playlists for training export:", mood_playlists.len());
    for (mood, pl_id) in &mood_playlists {
        log::info!("  - Mood '{}' (Playlist ID {})", mood, pl_id);
    }

    // Collect all unique tracks across these playlists
    let mut track_mood_labels: HashMap<i64, HashMap<String, u8>> = HashMap::new();
    let mut track_paths: HashMap<i64, String> = HashMap::new();
    let mut track_meta: HashMap<i64, (String, String)> = HashMap::new();

    for (mood, &pl_id) in &mood_playlists {
        let tracks = db.get_tracks_by_playlist(pl_id).map_err(|e| e.to_string())?;
        for tr in tracks {
            track_paths.insert(tr.id, tr.path.to_string());
            track_meta.insert(tr.id, (tr.artist.to_string(), tr.album.to_string()));
            let labels = track_mood_labels.entry(tr.id).or_default();
            labels.insert(mood.clone(), 1);
        }
    }

    let extractor = AudioFeatureExtractor::new();
    let mut csv_file =
        File::create(output_csv_path).map_err(|e| format!("Failed to create CSV file: {}", e))?;

    // Write CSV Header
    let mut header = String::from("song_id,title,artist,album,tempo,rms_mean,rms_std,zcr_mean,zcr_std,spectral_centroid_mean,spectral_centroid_std,spectral_rolloff_mean,spectral_rolloff_std,spectral_flatness_mean,spectral_flatness_std,spectral_flux_mean,spectral_flux_std,hpr,spectral_contrast_mean,spectral_contrast_std,crest_factor,mode_major_ratio");
    for i in 1..=13 {
        header.push_str(&format!(",mfcc_{}_mean,mfcc_{}_std", i, i));
    }
    for i in 1..=12 {
        header.push_str(&format!(",chroma_{}", i));
    }
    for mood in &target_moods {
        header.push_str(&format!(",{}", mood));
    }
    header.push('\n');
    csv_file
        .write_all(header.as_bytes())
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    let total_tracks = track_paths.len();
    log::info!(
        "Extracting features in parallel across all CPU cores for {} tracks...",
        total_tracks
    );

    let processed_counter = AtomicUsize::new(0);
    let track_items: Vec<(&i64, &String)> = track_paths.iter().collect();

    let csv_rows: Vec<Option<String>> = track_items
        .into_par_iter()
        .map(|(&track_id, path_str)| {
            let count = processed_counter.fetch_add(1, Ordering::SeqCst) + 1;
            log::info!("[{}/{}] Analyzing track ID {}...", count, total_tracks, track_id);

            // Check SQLite cache first
            let features = match db.get_audio_features(track_id) {
                Ok(Some(feat)) => feat,
                _ => {
                    if Path::new(path_str).exists() {
                        match extractor.extract_from_file(track_id, path_str) {
                            Ok(feat) => {
                                let _ = db.save_audio_features(&feat);
                                feat
                            }
                            Err(e) => {
                                log::warn!("Skipping track {}: {}", path_str, e);
                                return None;
                            }
                        }
                    } else {
                        log::warn!("Audio file not found: {}", path_str);
                        return None;
                    }
                }
            };

            let flattened = analysis::mood_classifier::flatten_features(&features);
            let title_clean = Path::new(path_str)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .replace(',', "_");

            let (artist, album) = track_meta
                .get(&track_id)
                .cloned()
                .unwrap_or_else(|| ("Unknown".into(), "Unknown".into()));
            let artist_clean = artist.replace(',', "_");
            let album_clean = album.replace(',', "_");

            let mut line = format!("{},{},{},{}", track_id, title_clean, artist_clean, album_clean);
            for val in &flattened {
                line.push_str(&format!(",{}", val));
            }

            let empty_labels = HashMap::new();
            let labels = track_mood_labels.get(&track_id).unwrap_or(&empty_labels);
            for mood in &target_moods {
                let val = labels.get(*mood).copied().unwrap_or(0);
                line.push_str(&format!(",{}", val));
            }
            line.push('\n');

            Some(line)
        })
        .collect();

    let mut exported_count = 0;
    for row in csv_rows.into_iter().flatten() {
        csv_file
            .write_all(row.as_bytes())
            .map_err(|e| format!("Failed to write CSV row: {}", e))?;
        exported_count += 1;
    }

    log::info!("Successfully exported {} tracks to {}", exported_count, output_csv_path);
    Ok(())
}

pub fn classify_all_tracks(db: &PlayTuneDb, model_json_path: &str) -> Result<(), String> {
    log::info!("Starting mood classification using model: {}", model_json_path);
    if !Path::new(model_json_path).exists() {
        return Err(format!(
            "Mood model weights file '{}' not found. Run training first.",
            model_json_path
        ));
    }

    let json_str = std::fs::read_to_string(model_json_path)
        .map_err(|e| format!("Failed to read model JSON: {}", e))?;

    let model = analysis::MoodClassifierModel::from_json(&json_str)
        .map_err(|e| format!("Failed to parse mood classifier model: {}", e))?;

    let tracks = db.get_all_tracks().map_err(|e| e.to_string())?;
    let total_tracks = tracks.len();

    if total_tracks == 0 {
        log::info!("No tracks found in the database to classify.");
        return Ok(());
    }

    log::info!(
        "Classifying moods for {} tracks using model '{}'...",
        total_tracks,
        model_json_path
    );

    let extractor = AudioFeatureExtractor::new();
    let mut classified_count = 0;

    for (idx, track) in tracks.iter().enumerate() {
        log::debug!(
            "[{}/{}] Classifying track ID {} ({})...",
            idx + 1,
            total_tracks,
            track.id,
            track.title
        );

        // 1. Get or compute acoustic features
        let features = match db.get_audio_features(track.id) {
            Ok(Some(feat)) => feat,
            _ => {
                if Path::new(track.path.as_ref()).exists() {
                    match extractor.extract_from_file(track.id, track.path.as_ref()) {
                        Ok(feat) => {
                            let _ = db.save_audio_features(&feat);
                            feat
                        }
                        Err(e) => {
                            log::warn!("Skipping feature extraction for {}: {}", track.path, e);
                            continue;
                        }
                    }
                } else {
                    log::warn!("Audio file not found: {}", track.path);
                    continue;
                }
            }
        };

        // 2. Classify mood scores
        let scores = model.classify(&features);

        // 3. Save mood scores in DB
        if let Err(e) = db.save_mood_scores(&scores) {
            log::warn!("Failed to save mood scores for track {}: {}", track.id, e);
        } else {
            classified_count += 1;
        }
    }

    log::info!("Successfully classified moods for {} / {} tracks!", classified_count, total_tracks);
    Ok(())
}

//! Library management — scanning, metadata, and cover art

mod cover_art;
pub mod loudness_scanner;
mod metadata;
pub mod playlist_io;
pub mod tag_editor;
pub use tag_editor::*;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant, UNIX_EPOCH},
};

use config::LibraryConfig;
pub use cover_art::{detect_image_mime, CoverArtData};
use db::{models::Track, PlayTuneDb as Database};
use log::{info, warn};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Database error: {0}")]
    Database(#[from] db::DbError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Scan cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

/// Supported audio file extensions
const AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "flac", "ogg", "opus", "wav", "aac", "m4a", "wma", "aiff", "ape", "alac"];

/// Minimum interval between progress callback invocations.
///
/// For large libraries, invoking the callback for every file can overwhelm
/// the UI thread. We throttle to at most one call per this duration.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(100);

/// Progress information during a library scan
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub files_found: u32,
    pub files_processed: u32,
    pub files_added: u32,
    pub files_updated: u32,
    pub files_removed: u32,
    /// Number of files that failed to be processed (parse/IO errors, batch failures).
    pub files_failed: u32,
    pub current_path: String,
}

/// Result of processing a single file during a scan
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileChange {
    Added,
    Updated,
    Unchanged,
}

/// RAII guard that resets the scanning flag on drop
struct ScanGuard {
    scanning: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.scanning.store(false, Ordering::Release);
    }
}

/// Metadata tags extracted from an audio file via symphonia's metadata API.
#[derive(Debug, Clone, Default)]
pub struct FileTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub lyrics: Option<String>,
}

/// Library scanner and manager
pub struct LibraryManager {
    db: Arc<Database>,
    config: LibraryConfig,
    /// Cancellation flag — set by `cancel_scan()`, read inside the scan loop.
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    /// Guard against concurrent scans — set to true while a scan is running.
    scanning: Arc<std::sync::atomic::AtomicBool>,
}

impl LibraryManager {
    pub fn new(db: Arc<Database>, config: LibraryConfig) -> Self {
        Self {
            db,
            config,
            cancel_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            scanning: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Check if a file extension is a supported audio format.
    pub fn is_audio_file(path: &Path) -> bool {
        // Fast path: extension match.
        if path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
        {
            return true;
        }
        let ext_is_unknown = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| !AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(true);
        if !ext_is_unknown {
            return false;
        }
        // Read the first 16 bytes and check against known magic numbers.
        // List derived from the typical signatures of each supported format.
        let Ok(mut file) = std::fs::File::open(path) else { return false };
        use std::io::Read;
        let mut buf = [0u8; 16];
        let n = file.read(&mut buf).unwrap_or(0);
        if n < 4 {
            return false;
        }
        // Magic-number table. Keep entries sorted by prefix length so we
        // check the most specific signatures first.
        if &buf[0..3] == b"ID3" {
            return true;
        }
        if &buf[0..4] == b"fLaC" {
            return true;
        }
        if &buf[0..4] == b"OggS" {
            return true;
        }
        if &buf[0..4] == b"RIFF" && n >= 12 && &buf[8..12] == b"WAVE" {
            return true;
        }
        if n >= 8 && &buf[4..8] == b"ftyp" {
            return true;
        } // M4A / ALAC / ISO-BMFF
        if (buf[0] == 0xFF) && ((buf[1] & 0xF6) == 0xF0) {
            return true;
        } // AAC ADTS
        false
    }

    /// Scan the library directories for new and modified files.
    pub fn scan<F: Fn(ScanProgress)>(
        &self,
        progress_callback: F,
    ) -> Result<ScanProgress, LibraryError> {
        self.cancel_flag.store(false, Ordering::Release);

        // Prevent concurrent scans
        if self.scanning.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err()
        {
            return Err(LibraryError::Other("A scan is already in progress".to_string()));
        }
        let _guard = ScanGuard { scanning: Arc::clone(&self.scanning) };

        let mut progress = ScanProgress {
            files_found: 0,
            files_processed: 0,
            files_added: 0,
            files_updated: 0,
            files_removed: 0,
            files_failed: 0,
            current_path: String::new(),
        };

        // Pre-load existing (path → file_modified) for O(1) lookups
        let existing_tracks: HashMap<String, i64> = self
            .db
            .get_tracks_with_mtime()?
            .into_iter()
            .map(|(_id, path, mtime)| (path, mtime))
            .collect();

        let mut folder_cache: HashMap<String, i64> = self
            .db
            .get_all_folders()
            .unwrap_or_default()
            .into_iter()
            .map(|f| (f.path, f.id))
            .collect();

        let mut audio_files: Vec<PathBuf> = Vec::new();
        for dir in &self.config.watch_dirs {
            let mut walker = WalkDir::new(dir);
            if let Some(d) = self.config.max_depth {
                walker = walker.max_depth(d);
            }
            // Pre-compile exclude glob patterns once per watch_dir.
            let exclude_patterns: Vec<glob::Pattern> = self
                .config
                .exclude_globs
                .iter()
                .filter_map(|g| glob::Pattern::new(g).ok())
                .collect();

            for entry in walker.into_iter().filter_entry(|e| {
                if exclude_patterns.iter().any(|p| p.matches_path(e.path())) {
                    return false;
                }
                true
            }) {
                match entry {
                    Ok(e) => {
                        let path = e.path();
                        if path.is_file() && Self::is_audio_file(path) {
                            audio_files.push(path.to_path_buf());
                            progress.files_found += 1;
                        }
                    }
                    Err(e) => {
                        warn!("Walkdir error in {}: {}", dir.display(), e);
                    }
                }
                if self.cancel_flag.load(Ordering::Acquire) {
                    return Err(LibraryError::Cancelled);
                }
            }
        }

        info!("Found {} audio files", audio_files.len());

        const BATCH_SIZE: usize = 250;
        let mut new_tracks: Vec<Track> = Vec::with_capacity(BATCH_SIZE);
        let mut updated_tracks: Vec<Track> = Vec::with_capacity(BATCH_SIZE);
        // Cover art pre-extracted alongside new_tracks; indices match
        let mut pending_cover_art: Vec<Option<CoverArtData>> = Vec::with_capacity(BATCH_SIZE);
        // Cover art ready to persist after album IDs are known
        let mut cover_art_queue: Vec<(PathBuf, i64, CoverArtData)> = Vec::new();

        let mut last_callback = Instant::now();

        for path in &audio_files {
            if self.cancel_flag.load(Ordering::Acquire) {
                return Err(LibraryError::Cancelled);
            }

            // Throttled progress callback
            let now = Instant::now();
            if now.duration_since(last_callback) >= PROGRESS_THROTTLE {
                progress.current_path = path.to_string_lossy().into_owned();
                progress_callback(progress.clone());
                last_callback = now;
            }

            let file_metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to read metadata for {}: {}", path.display(), e);
                    progress.files_processed += 1;
                    progress.files_failed += 1;
                    continue;
                }
            };
            let file_size = i64::try_from(file_metadata.len()).unwrap_or(i64::MAX);
            let file_modified = file_metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let path_str = path.to_string_lossy().into_owned();

            if let Some(&existing_mtime) = existing_tracks.get(&path_str) {
                if existing_mtime >= file_modified {
                    progress.files_processed += 1;
                    continue;
                }
                // Modified file — update, skip cover re-extraction
                match self.extract_track_info(path, file_size, file_modified) {
                    Ok(mut track) => {
                        track.folder_id = self.lookup_folder_id_with_cache(path, &mut folder_cache);
                        updated_tracks.push(track);
                    }
                    Err(e) => {
                        warn!("Failed to extract info for {}: {}", path.display(), e);
                        progress.files_processed += 1;
                        progress.files_failed += 1;
                        continue;
                    }
                }
            } else {
                // New file — combined probe for metadata + cover art
                match self.extract_track_info_with_cover(path, file_size, file_modified) {
                    Ok((mut track, cover)) => {
                        track.folder_id = self.lookup_folder_id_with_cache(path, &mut folder_cache);
                        new_tracks.push(track);
                        pending_cover_art.push(cover);
                    }
                    Err(e) => {
                        warn!("Failed to extract info for {}: {}", path.display(), e);
                        progress.files_processed += 1;
                        progress.files_failed += 1;
                        continue;
                    }
                }
            }

            progress.files_processed += 1;

            if new_tracks.len() >= BATCH_SIZE {
                self.flush_new_batch(
                    &mut new_tracks,
                    &mut pending_cover_art,
                    &mut cover_art_queue,
                    &mut progress,
                );
            }
            if updated_tracks.len() >= BATCH_SIZE {
                self.flush_updated_batch(&mut updated_tracks, &mut progress);
            }
        }

        if !new_tracks.is_empty() {
            self.flush_new_batch(
                &mut new_tracks,
                &mut pending_cover_art,
                &mut cover_art_queue,
                &mut progress,
            );
        }
        if !updated_tracks.is_empty() {
            self.flush_updated_batch(&mut updated_tracks, &mut progress);
        }

        let existing_paths: Vec<String> =
            audio_files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        let existing_refs: Vec<&str> = existing_paths.iter().map(|s| s.as_str()).collect();
        match self.db.cleanup_missing_tracks(&existing_refs) {
            Ok(removed) => {
                progress.files_removed = removed as u32;
                info!("Removed {} tracks with missing files", removed);
            }
            Err(e) => warn!("Failed to cleanup missing tracks: {}", e),
        }

        info!(
            "Scan complete: {} added, {} updated, {} removed, {} failed",
            progress.files_added,
            progress.files_updated,
            progress.files_removed,
            progress.files_failed
        );

        // Final callback so UI reaches 100%
        progress.current_path = String::new();
        progress_callback(progress.clone());

        Ok(progress)
    }

    fn flush_new_batch(
        &self,
        new_tracks: &mut Vec<Track>,
        pending_cover_art: &mut Vec<Option<CoverArtData>>,
        cover_art_queue: &mut Vec<(PathBuf, i64, CoverArtData)>,
        progress: &mut ScanProgress,
    ) {
        // Build the slice of tuples that insert_tracks_batch_tx expects.
        // We borrow from new_tracks so no cloning of strings is needed.
        let batch: Vec<(
            &str,
            &str,
            &str,
            &str,
            f64,
            &str,
            Option<i64>,
            i64,
            Option<&str>,
            Option<&str>,
            Option<i32>,
        )> = new_tracks
            .iter()
            .map(|t| {
                (
                    t.path.as_str(),
                    if t.title.is_empty() { "Unknown" } else { t.title.as_str() },
                    t.artist.as_deref().unwrap_or("Unknown"),
                    t.album.as_deref().unwrap_or("Unknown"),
                    t.duration_secs as f64,
                    t.duration_str.as_str(),
                    t.folder_id,
                    t.file_modified,
                    t.lyrics_synced.as_deref(),
                    t.lyrics_unsynced.as_deref(),
                    t.track_number,
                )
            })
            .collect();

        // Single transaction for the entire batch — one WAL sync instead of N.
        let ids_result =
            self.db.with_transaction(|tx| Database::insert_tracks_batch_tx(tx, &batch));

        match ids_result {
            Ok(ids) => {
                progress.files_added += ids.len() as u32;
                // Post-commit: persist cover art for each newly inserted track.
                for (orig_idx, track_id) in ids {
                    if let Some(slot) = pending_cover_art.get_mut(orig_idx) {
                        if let Some(art) = slot.take() {
                            let album_id =
                                self.db.get_track(track_id).ok().flatten().and_then(|t| {
                                    let album = t.album;
                                    if album.is_empty() {
                                        None
                                    } else {
                                        self.db.get_album_id(&album, None).ok().flatten()
                                    }
                                });
                            if let Err(e) = self.db.insert_cover_art(
                                album_id,
                                Some(track_id),
                                None,
                                Some(&art.data),
                                Some(&art.data_hash),
                                art.width,
                                art.height,
                                &art.mime_type,
                            ) {
                                warn!("Failed to persist cover art for track {}: {}", track_id, e);
                                progress.files_failed += 1;
                            }
                            // Write cover to the file-system cache so the GUI
                            // can display it without re-reading the audio file.
                            if let Some(cache_dir) = dirs::cache_dir() {
                                let mut cache_path = cache_dir;
                                cache_path.push("playtune");
                                cache_path.push("covers");
                                let _ = std::fs::create_dir_all(&cache_path);
                                let ext = if art.mime_type.contains("png") {
                                    "png"
                                } else if art.mime_type.contains("webp") {
                                    "webp"
                                } else {
                                    "jpg"
                                };
                                cache_path.push(format!("{}.{}", track_id, ext));
                                if let Err(e) = std::fs::write(&cache_path, &art.data) {
                                    warn!(
                                        "Failed to write cover cache for track {}: {}",
                                        track_id, e
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // The entire batch failed (e.g., disk full). Mark every
                // track in this batch as failed so progress is accurate.
                log::warn!("Failed to insert batch of {} tracks: {}", new_tracks.len(), e);
                progress.files_failed += new_tracks.len() as u32;
            }
        }
        new_tracks.clear();
        pending_cover_art.clear();
        cover_art_queue.clear();
    }

    fn flush_updated_batch(&self, updated_tracks: &mut Vec<Track>, progress: &mut ScanProgress) {
        // Single transaction wraps all UPDATE statements — one WAL fsync
        // instead of N (critical for HDD performance on large libraries).
        let batch: Vec<(
            &str,
            &str,
            &str,
            &str,
            f64,
            &str,
            Option<i64>,
            i64,
            Option<&str>,
            Option<&str>,
            Option<i32>,
        )> = updated_tracks
            .iter()
            .map(|t| {
                (
                    t.path.as_str(),
                    if t.title.is_empty() { "Unknown" } else { t.title.as_str() },
                    t.artist.as_deref().unwrap_or("Unknown"),
                    t.album.as_deref().unwrap_or("Unknown"),
                    t.duration_secs as f64,
                    t.duration_str.as_str(),
                    t.folder_id,
                    t.file_modified,
                    t.lyrics_synced.as_deref(),
                    t.lyrics_unsynced.as_deref(),
                    t.track_number,
                )
            })
            .collect();

        match self.db.with_transaction(|tx| Database::insert_tracks_batch_tx(tx, &batch)) {
            Ok(ids) => progress.files_updated += ids.len() as u32,
            Err(e) => {
                log::warn!("Failed to update batch of {} tracks: {}", updated_tracks.len(), e);
                progress.files_failed += updated_tracks.len() as u32;
            }
        }
        updated_tracks.clear();
    }

    /// Cancel an in-progress scan.
    pub fn cancel_scan(&self) {
        self.cancel_flag.store(true, Ordering::Release);
    }

    /// Update the configuration. Must not be called while a scan is in progress.
    pub fn set_config(&mut self, config: LibraryConfig) {
        self.config = config;
    }

    /// Read-only access to the current configuration. Used by callers that
    /// need the list of watch directories (e.g. the M3U importer, which
    /// resolves relative paths against each watch_dir).
    pub fn config(&self) -> &LibraryConfig {
        &self.config
    }

    /// Scan a list of individual audio files and insert them into the database.
    pub fn scan_files(&self, paths: &[std::path::PathBuf]) -> usize {
        // Build the folder cache ONCE before the loop so we don't re-query the
        // DB for every file (old `lookup_folder_id` did exactly that).
        let mut folder_cache: HashMap<String, i64> = HashMap::new();
        if let Ok(folders) = self.db.get_all_folders() {
            for f in folders {
                folder_cache.insert(f.path, f.id);
            }
        }

        let mut added = 0usize;
        for path in paths {
            if !path.is_file() || !Self::is_audio_file(path) {
                continue;
            }
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to read metadata for {}: {}", path.display(), e);
                    continue;
                }
            };
            let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            let mtime = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            if let Ok((mut track, cover)) = self.extract_track_info_with_cover(path, size, mtime) {
                track.folder_id = self.lookup_folder_id_with_cache(path, &mut folder_cache);
                if self
                    .db
                    .add_or_update_track_with_lyrics(
                        &track.path,
                        if track.title.is_empty() { "Unknown" } else { &track.title },
                        track.artist.as_deref().unwrap_or("Unknown"),
                        track.album.as_deref().unwrap_or("Unknown"),
                        track.duration_secs as f64,
                        &track.duration_str,
                        track.folder_id,
                        track.file_modified,
                        track.lyrics_synced.as_deref(),
                        track.lyrics_unsynced.as_deref(),
                        track.track_number,
                    )
                    .is_ok()
                {
                    if cover.is_none() {
                        let _ = engine::extract_cover_art_to_cache(path);
                    }
                    added += 1;
                } else {
                    warn!("Failed to insert track {}", path.display());
                }
            }
        }
        added
    }

    /// Look up the folder_id for a track's parent directory by matching
    /// against configured watch_dirs. Returns None if no folder matches.
    fn lookup_folder_id_with_cache(
        &self,
        path: &Path,
        folder_cache: &mut HashMap<String, i64>,
    ) -> Option<i64> {
        let parent = path.parent()?;
        for dir in &self.config.watch_dirs {
            if let Ok(_rel) = parent.strip_prefix(dir) {
                let folder_path = dir.to_string_lossy().into_owned();
                if let Some(&id) = folder_cache.get(&folder_path) {
                    return Some(id);
                }
                // Cache miss: lazily register the watch_dir and cache the id.
                let folder_name =
                    dir.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
                if let Ok(id) = self.db.add_folder(&folder_path, &folder_name, 0) {
                    folder_cache.insert(folder_path, id);
                    return Some(id);
                }
                return None;
            }
        }
        None
    }

    /// Legacy `lookup_folder_id` kept for callers that don't have a
    /// pre-seeded cache. Builds a fresh folder map on every call.
    /// Prefer `lookup_folder_id_with_cache` when calling in a loop.
    #[allow(dead_code)]
    fn lookup_folder_id(&self, path: &Path) -> Option<i64> {
        let mut cache: HashMap<String, i64> = HashMap::new();
        // Pre-seed with all existing folders so we don't re-add folders
        // that already exist.
        if let Ok(folders) = self.db.get_all_folders() {
            for f in folders {
                cache.insert(f.path, f.id);
            }
        }
        self.lookup_folder_id_with_cache(path, &mut cache)
    }
}

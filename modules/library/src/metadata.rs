//! Metadata extraction pipeline for audio files.

use db::models::Track;
use log::warn;
use std::path::Path;

use super::{CoverArtData, FileTags, LibraryError};

impl super::LibraryManager {
    pub(crate) fn extract_track_info(
        &self,
        path: &Path,
        file_size: i64,
        file_modified: i64,
    ) -> Result<Track, LibraryError> {
        let (dur, sr, ch, tags, _cover) = Self::probe_file(path).ok_or_else(|| {
            LibraryError::Other(format!("Could not probe audio info for {}", path.display()))
        })?;
        self.build_track(path, file_size, file_modified, dur, sr, ch, tags)
    }

    pub(crate) fn extract_track_info_with_cover(
        &self,
        path: &Path,
        file_size: i64,
        file_modified: i64,
    ) -> Result<(Track, Option<CoverArtData>), LibraryError> {
        let (dur, sr, ch, tags, cover) = Self::probe_file(path).ok_or_else(|| {
            LibraryError::Other(format!("Could not probe audio info for {}", path.display()))
        })?;
        let track = self.build_track(path, file_size, file_modified, dur, sr, ch, tags)?;
        Ok((track, cover))
    }

    pub(crate) fn probe_file(
        path: &Path,
    ) -> Option<(f32, u32, usize, FileTags, Option<CoverArtData>)> {
        use symphonia::core::{
            codecs::CODEC_TYPE_NULL, formats::FormatOptions, io::MediaSourceStream,
            meta::MetadataOptions, probe::Hint,
        };

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("probe_file: cannot open {}: {}", path.display(), e);
                return None;
            }
        };
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut probed = match symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        ) {
            Ok(p) => p,
            Err(e) => {
                log::warn!(
                    "probe_file: symphonia format probing failed for {}: {}",
                    path.display(),
                    e
                );
                return None;
            }
        };

        let track =
            match probed.format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL) {
                Some(t) => t,
                None => {
                    log::warn!("probe_file: no playable audio track found in {}", path.display());
                    return None;
                }
            };

        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(0);
        let channels = codec_params.channels.map(|c| c.count()).unwrap_or(0);

        if sample_rate == 0 || channels == 0 {
            warn!(
                "File {} missing codec parameters (sample_rate={}, channels={}) — skipping",
                path.display(),
                sample_rate,
                channels
            );
            return None;
        }

        let duration =
            codec_params.n_frames.map(|n| n as f32 / sample_rate.max(1) as f32).unwrap_or(-1.0);

        let (tags, cover_art) = Self::extract_tags_and_cover_from_probed(&mut probed);
        Some((duration, sample_rate, channels, tags, cover_art))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn build_track(
        &self,
        path: &Path,
        file_size: i64,
        file_modified: i64,
        duration_secs: f32,
        sample_rate: u32,
        channels: usize,
        tags: FileTags,
    ) -> Result<Track, LibraryError> {
        debug_assert!(
            sample_rate > 0,
            "build_track called with sample_rate == 0; probe_file should have rejected this file"
        );

        let bitrate_kbps = if duration_secs > 0.0 {
            let raw = (file_size as f32 * 8.0) / duration_secs / 1000.0;
            Some(raw.round().clamp(8.0, 10000.0) as i32)
        } else {
            None
        };

        let stored_duration = if duration_secs < 0.0 {
            warn!("Duration unknown for {} (n_frames unavailable); storing 0.0", path.display());
            0.0_f32
        } else {
            duration_secs
        };

        let title = tags.title.unwrap_or_else(|| match path.file_stem().and_then(|s| s.to_str()) {
            Some(stem) => stem.to_string(),
            None => {
                warn!(
                    "Non-UTF-8 filename for {}; falling back to title \"Unknown\"",
                    path.display()
                );
                "Unknown".to_string()
            }
        });

        let total_secs = stored_duration.max(0.0) as u64;
        let duration_str = if total_secs >= 3600 {
            format!("{}:{:02}:{:02}", total_secs / 3600, (total_secs / 60) % 60, total_secs % 60)
        } else {
            format!("{}:{:02}", total_secs / 60, total_secs % 60)
        };

        let (lyrics_synced, lyrics_unsynced) =
            Self::extract_lyrics_for_track(path, tags.lyrics.as_deref());

        Ok(Track {
            id: 0,
            path: std::sync::Arc::from(path.to_string_lossy()),
            title: std::sync::Arc::from(title),
            artist: tags.artist.map(std::sync::Arc::from),
            album: tags.album.map(std::sync::Arc::from),
            album_artist: tags.album_artist.map(std::sync::Arc::from),
            genre: tags.genre.map(std::sync::Arc::from),
            year: tags.year,
            track_number: tags.track_number,
            disc_number: tags.disc_number,
            duration_secs: stored_duration,
            duration_str: std::sync::Arc::from(duration_str),
            sample_rate: sample_rate as i32,
            channels: channels as i32,
            bitrate_kbps,
            format: std::sync::Arc::from(
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            file_size,
            file_modified,
            crc32: None,
            replaygain_track_db: None,
            replaygain_album_db: None,
            replaygain_track_peak: None,
            replaygain_album_peak: None,
            ebu_r128_loudness: None,
            ebu_r128_peak: None,
            bpm: None,
            lyrics_synced: lyrics_synced.map(std::sync::Arc::from),
            lyrics_unsynced: lyrics_unsynced.map(std::sync::Arc::from),
            rating: 0,
            last_played: None,
            play_count: 0,
            date_added: chrono::Utc::now().naive_utc(),
            date_scanned: chrono::Utc::now().naive_utc(),
            folder_id: None,
        })
    }

    pub(crate) fn extract_tags_and_cover_from_probed(
        probed: &mut symphonia::core::probe::ProbeResult,
    ) -> (FileTags, Option<CoverArtData>) {
        let mut tags = FileTags::default();
        let mut cover: Option<CoverArtData> = None;

        if let Some(mut metadata) = probed.metadata.get() {
            if let Some(rev) = metadata.current() {
                Self::read_tags_from_revision(rev, &mut tags);
                if cover.is_none() {
                    cover = Self::extract_visual_from_revision(rev);
                }
            }
            if let Some(rev) = metadata.skip_to_latest() {
                Self::read_tags_from_revision(rev, &mut tags);
                if cover.is_none() {
                    cover = Self::extract_visual_from_revision(rev);
                }
            }
        }
        {
            let mut fmt_meta = probed.format.metadata();
            if let Some(rev) = fmt_meta.current() {
                Self::read_tags_from_revision(rev, &mut tags);
                if cover.is_none() {
                    cover = Self::extract_visual_from_revision(rev);
                }
            }
            if let Some(rev) = fmt_meta.skip_to_latest() {
                Self::read_tags_from_revision(rev, &mut tags);
                if cover.is_none() {
                    cover = Self::extract_visual_from_revision(rev);
                }
            }
        }
        (tags, cover)
    }

    pub(crate) fn read_tags_from_revision(
        revision: &symphonia::core::meta::MetadataRevision,
        tags: &mut FileTags,
    ) {
        use symphonia::core::meta::StandardTagKey;
        for tag in revision.tags() {
            if let Some(std_key) = tag.std_key {
                match std_key {
                    StandardTagKey::TrackTitle if tags.title.is_none() => {
                        tags.title = tag_value_to_string(&tag.value);
                    }
                    StandardTagKey::Artist if tags.artist.is_none() => {
                        tags.artist = tag_value_to_string(&tag.value);
                    }
                    StandardTagKey::Album if tags.album.is_none() => {
                        tags.album = tag_value_to_string(&tag.value);
                    }
                    StandardTagKey::AlbumArtist if tags.album_artist.is_none() => {
                        tags.album_artist = tag_value_to_string(&tag.value);
                    }
                    StandardTagKey::Genre if tags.genre.is_none() => {
                        tags.genre = tag_value_to_string(&tag.value);
                    }
                    StandardTagKey::Date if tags.year.is_none() => {
                        tags.year = tag_value_to_year(&tag.value);
                    }
                    StandardTagKey::TrackNumber if tags.track_number.is_none() => {
                        tags.track_number = tag_value_to_i32(&tag.value);
                    }
                    StandardTagKey::DiscNumber if tags.disc_number.is_none() => {
                        tags.disc_number = tag_value_to_i32(&tag.value);
                    }
                    StandardTagKey::Lyrics if tags.lyrics.is_none() => {
                        tags.lyrics = tag_value_to_string(&tag.value);
                    }
                    _ => {}
                }
            }
            if tags.lyrics.is_none() {
                let k_upper = tag.key.to_uppercase();
                if k_upper == "LYRICS"
                    || k_upper == "UNSYNCEDLYRICS"
                    || k_upper == "USLT"
                    || k_upper == "SYLT"
                {
                    tags.lyrics = tag_value_to_string(&tag.value);
                }
            }
        }
    }

    pub(crate) fn extract_lyrics_for_track(
        path: &Path,
        embedded_lyrics: Option<&str>,
    ) -> (Option<String>, Option<String>) {
        use lofty::prelude::*;
        let is_synced = |s: &str| s.contains('[') && s.contains(':') && s.contains(']');

        // 1. Check local sidecar .lrc file first
        let lrc_path = path.with_extension("lrc");
        if let Ok(content) = std::fs::read_to_string(&lrc_path) {
            let content = content.trim();
            if !content.is_empty() {
                if is_synced(content) {
                    return (Some(content.to_string()), None);
                } else {
                    return (None, Some(content.to_string()));
                }
            }
        }

        // 2. Check embedded lyrics from symphonia tags
        if let Some(content) = embedded_lyrics {
            let content = content.trim();
            if !content.is_empty() {
                if is_synced(content) {
                    return (Some(content.to_string()), None);
                } else {
                    return (None, Some(content.to_string()));
                }
            }
        }

        // 3. Fallback: check lofty tags if embedded symphonia tags missed it
        if let Ok(tagged_file) = lofty::read_from_path(path) {
            for tag in tagged_file.tags() {
                if let Some(content) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                    let content = content.trim();
                    if !content.is_empty() {
                        if is_synced(content) {
                            return (Some(content.to_string()), None);
                        } else {
                            return (None, Some(content.to_string()));
                        }
                    }
                }
            }
        }

        (None, None)
    }

    pub fn read_file_tags(path: &Path) -> Option<FileTags> {
        Self::probe_file(path).map(|(_, _, _, tags, _)| tags)
    }
}

pub(crate) fn tag_value_to_string(value: &symphonia::core::meta::Value) -> Option<String> {
    match value {
        symphonia::core::meta::Value::String(s) => Some(s.clone()),
        symphonia::core::meta::Value::UnsignedInt(u) => Some(u.to_string()),
        symphonia::core::meta::Value::SignedInt(i) => Some(i.to_string()),
        symphonia::core::meta::Value::Float(f) => Some(f.to_string()),
        _ => None,
    }
}

pub(crate) fn tag_value_to_i32(value: &symphonia::core::meta::Value) -> Option<i32> {
    match value {
        symphonia::core::meta::Value::SignedInt(i) => i32::try_from(*i).ok(),
        symphonia::core::meta::Value::UnsignedInt(u) => i32::try_from(*u).ok(),
        symphonia::core::meta::Value::String(s) => s.parse::<i32>().ok(),
        _ => None,
    }
}

pub(crate) fn tag_value_to_year(value: &symphonia::core::meta::Value) -> Option<i32> {
    match value {
        symphonia::core::meta::Value::SignedInt(i) => i32::try_from(*i).ok(),
        symphonia::core::meta::Value::UnsignedInt(u) => i32::try_from(*u).ok(),
        symphonia::core::meta::Value::String(s) => {
            if let Ok(y) = s.parse::<i32>() {
                return Some(y);
            }
            s.split('-').next().and_then(|p| p.parse::<i32>().ok())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::LibraryManager;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_extract_sidecar_lrc_synced() {
        let dir = std::env::temp_dir();
        let mp3_path = dir.join("test_sidecar_synced_4e83.mp3");
        let lrc_path = dir.join("test_sidecar_synced_4e83.lrc");

        File::create(&mp3_path).unwrap();
        let mut lrc_file = File::create(&lrc_path).unwrap();
        writeln!(lrc_file, "[00:12.34] Synced line 1\n[00:15.00] Synced line 2").unwrap();

        let (synced, unsynced) = LibraryManager::extract_lyrics_for_track(&mp3_path, None);
        assert!(synced.is_some());
        assert!(unsynced.is_none());
        assert!(synced.unwrap().contains("[00:12.34] Synced line 1"));

        let _ = std::fs::remove_file(&mp3_path);
        let _ = std::fs::remove_file(&lrc_path);
    }

    #[test]
    fn test_extract_sidecar_lrc_unsynced() {
        let dir = std::env::temp_dir();
        let mp3_path = dir.join("test_sidecar_unsynced_4e83.mp3");
        let lrc_path = dir.join("test_sidecar_unsynced_4e83.lrc");

        File::create(&mp3_path).unwrap();
        let mut lrc_file = File::create(&lrc_path).unwrap();
        writeln!(lrc_file, "Just plain unsynced text without timestamps").unwrap();

        let (synced, unsynced) = LibraryManager::extract_lyrics_for_track(&mp3_path, None);
        assert!(synced.is_none());
        assert!(unsynced.is_some());
        assert_eq!(unsynced.unwrap(), "Just plain unsynced text without timestamps");

        let _ = std::fs::remove_file(&mp3_path);
        let _ = std::fs::remove_file(&lrc_path);
    }

    #[test]
    fn test_extract_embedded_lyrics() {
        let dir = std::env::temp_dir();
        let mp3_path = dir.join("test_embedded_4e83.mp3");
        File::create(&mp3_path).unwrap();

        let embedded = "[01:00.00] Embedded synced";
        let (synced, unsynced) =
            LibraryManager::extract_lyrics_for_track(&mp3_path, Some(embedded));
        assert!(synced.is_some());
        assert!(unsynced.is_none());
        assert_eq!(synced.unwrap(), "[01:00.00] Embedded synced");

        let _ = std::fs::remove_file(&mp3_path);
    }
}

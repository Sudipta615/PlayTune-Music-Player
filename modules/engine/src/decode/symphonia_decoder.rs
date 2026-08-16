//! Audio decoder using Symphonia for format support
//! Supports MP3, FLAC, OGG/Vorbis, WAV, AAC, and more
//! All decoding is off the audio thread and thread-safe
#![allow(clippy::incompatible_msrv)]

use std::{fs::File, path::Path};

use symphonia::core::{
    codecs::audio::{AudioDecoder, AudioDecoderOptions, CODEC_ID_NULL_AUDIO},
    errors::Error as SymphoniaError,
    formats::{probe::Hint, FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTag, StandardVisualKey},
    units::{Time, Timestamp},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Failed to open file: {0}")]
    FileOpen(String),
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Seek error: {0}")]
    Seek(String),
    #[error("End of stream")]
    EndOfStream,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Decoded audio format information
#[derive(Debug, Clone)]
pub struct DecodeInfo {
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_secs: f32,
    pub codec: String,
    pub bitrate_kbps: Option<u32>,
}

/// A chunk of decoded PCM audio
#[derive(Debug, Clone)]
pub struct DecodedChunk {
    /// Interleaved f32 samples (L, R, L, R, ...)
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
    pub frame_count: usize,
}

/// Symphonia-based audio decoder
pub struct SymphoniaDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    info: DecodeInfo,
    /// Reusable sample buffer for decoded output, passed across
    /// decode_next calls instead of allocating a new Vec each time.
    sample_buffer: Vec<f32>,
    /// Reusable scratch buffer for generic sample to f32 interleaved conversion
    scratch_interleaved: Vec<f32>,
}

impl SymphoniaDecoder {
    /// Open a file for decoding
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        let file = File::open(path)
            .map_err(|e| DecodeError::FileOpen(format!("Cannot open {}: {}", path.display(), e)))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = AudioDecoderOptions::default();

        let format_reader = symphonia::default::get_probe()
            .probe(&hint, mss, format_opts, metadata_opts)
            .map_err(|e| DecodeError::UnsupportedFormat(format!("Probe failed: {}", e)))?;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| {
                t.codec_params
                    .as_ref()
                    .and_then(|cp| cp.audio())
                    .is_some_and(|a| a.codec != CODEC_ID_NULL_AUDIO)
            })
            .ok_or_else(|| DecodeError::UnsupportedFormat("No audio track found".to_string()))?;

        let track_id = track.id;
        let audio_params =
            track.codec_params.as_ref().and_then(|cp| cp.audio()).ok_or_else(|| {
                DecodeError::UnsupportedFormat("No audio codec params".to_string())
            })?;

        if let Some(delay) = track.delay {
            log::info!("Gapless metadata found: {} samples delay", delay);
        }
        if let Some(padding) = track.padding {
            log::info!("Gapless metadata found: {} samples padding", padding);
        }

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(audio_params, &decoder_opts)
            .map_err(|e| DecodeError::Decode(format!("Cannot create decoder: {}", e)))?;

        let sample_rate = audio_params.sample_rate.unwrap_or(44100);
        let src_channels = audio_params.channels.as_ref().map(|c| c.count()).unwrap_or(2);
        if src_channels > 2 {
            log::warn!(
                "File has {} channels; tc-engine supports up to 2 channels. Only the first two channels will be used.",
                src_channels
            );
        }
        let channels = src_channels.min(2);

        // calculation to prevent overflow with extremely large frame counts
        // (e.g. on 32-bit targets where n_frames could be near usize::MAX).
        let duration_secs = track
            .num_frames
            .map(|n| {
                let n_frames = n as f32;
                let rate = sample_rate as f32;
                if rate > 0.0 {
                    n_frames / rate
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let codec = format!("{:?}", audio_params.codec);

        let info = DecodeInfo { sample_rate, channels, duration_secs, codec, bitrate_kbps: None };

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            info,
            sample_buffer: Vec::with_capacity(4096 * channels),
            scratch_interleaved: Vec::with_capacity(4096 * src_channels),
        })
    }

    /// Decode the next chunk of audio.
    ///
    /// Reuses the internal `sample_buffer` across calls instead of
    /// allocating a new one on every call.
    pub fn decode_next(&mut self, max_frames: usize) -> Result<DecodedChunk, DecodeError> {
        self.sample_buffer.clear();
        let mut frames_decoded = 0;
        let mut consecutive_skips = 0u32;
        const MAX_CONSECUTIVE_SKIPS: u32 = 32;

        while frames_decoded < max_frames {
            let packet = match self.format_reader.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => {
                    // End of stream
                    break;
                }
                Err(SymphoniaError::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                    consecutive_skips += 1;
                    if consecutive_skips > MAX_CONSECUTIVE_SKIPS {
                        log::debug!("Max consecutive ResetRequired skips reached near stream end");
                        break;
                    }
                    continue;
                }
                Err(SymphoniaError::IoError(_)) => {
                    // Generic IO error at stream end should break to trigger EndOfStream
                    break;
                }
                Err(e) => return Err(DecodeError::Decode(format!("Packet read error: {}", e))),
            };

            if packet.track_id != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let src_channels = decoded.num_planes();
                    let decoded_frames = decoded.frames();

                    self.scratch_interleaved.clear();
                    decoded.copy_to_vec_interleaved(&mut self.scratch_interleaved);

                    let frames = Self::extract_from_interleaved_f32(
                        &self.scratch_interleaved,
                        &mut self.sample_buffer,
                        src_channels,
                        self.info.channels,
                        decoded_frames,
                    );
                    frames_decoded += frames;
                    consecutive_skips = 0;
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    self.decoder.reset();
                    consecutive_skips += 1;
                    if consecutive_skips > MAX_CONSECUTIVE_SKIPS {
                        if self.sample_buffer.is_empty() {
                            return Err(DecodeError::EndOfStream);
                        }
                        break;
                    }
                    continue;
                }
                Err(e) => return Err(DecodeError::Decode(format!("Decode error: {}", e))),
            }
        }

        if self.sample_buffer.is_empty() {
            if consecutive_skips > 0 {
                log::debug!(
                    "End of stream reached after {} consecutive decode skips",
                    consecutive_skips
                );
            }
            return Err(DecodeError::EndOfStream);
        }
        let cap = self.sample_buffer.capacity();
        let samples = std::mem::replace(&mut self.sample_buffer, Vec::with_capacity(cap));

        Ok(DecodedChunk {
            samples,
            channels: self.info.channels,
            sample_rate: self.info.sample_rate,
            frame_count: frames_decoded,
        })
    }

    /// Extract f32 samples from an interleaved f32 slice and handle downmixing/upmixing
    fn extract_from_interleaved_f32(
        samples: &[f32],
        output: &mut Vec<f32>,
        src_channels: usize,
        target_channels: usize,
        frames: usize,
    ) -> usize {
        let actual_frames = (samples.len() / src_channels.max(1)).min(frames);
        if actual_frames < frames {
            log::warn!(
                "Decoder reported {} frames × {} channels = {} samples, but buffer has only {}; using {} frames",
                frames,
                src_channels,
                frames * src_channels,
                samples.len(),
                actual_frames
            );
        }
        let needed_samples = actual_frames * target_channels;
        output.reserve(needed_samples);

        if src_channels == 2 && target_channels == 2 {
            let available = samples.len() / 2;
            let copy_frames = available.min(actual_frames);
            let copy_samples = copy_frames * 2;
            output.extend_from_slice(&samples[..copy_samples]);
            return copy_frames;
        }

        if src_channels == 1 && target_channels == 2 {
            for frame in 0..actual_frames {
                let s = if frame < samples.len() { samples[frame] } else { 0.0 };
                output.push(s);
                output.push(s);
            }
            return actual_frames;
        }

        for frame in 0..actual_frames {
            let frame_offset = frame * src_channels;
            for ch in 0..target_channels {
                let sample = if ch < src_channels && frame_offset + ch < samples.len() {
                    samples[frame_offset + ch]
                } else if src_channels > 0 && frame_offset + src_channels - 1 < samples.len() {
                    samples[frame_offset + src_channels - 1]
                } else {
                    0.0
                };
                output.push(sample);
            }
        }
        actual_frames
    }

    /// Seek to a position in seconds
    pub fn seek(&mut self, position_secs: f32) -> Result<(), DecodeError> {
        // Defensive: clamp non-finite or out-of-range values. The engine's
        // Seek command handler already validates, but `decoder.seek()` is
        // also callable from other paths (e.g., test code, future
        // features). With `panic = "abort"` we cannot tolerate a panic
        // inside `Time::from` (which can panic on NaN/inf in some
        // symphonia versions).
        if !position_secs.is_finite() || position_secs < 0.0 {
            return Err(DecodeError::Seek(format!("Invalid seek position: {}", position_secs)));
        }
        // Clamp to a 24-hour upper bound. Symphonia's Time is u64-based
        // internally, but extremely large f32 values can overflow during
        // conversion. 24h is well beyond any real audio file.
        let clamped = position_secs.min(86400.0);
        let time = Time::try_from_secs_f64(clamped as f64).unwrap_or(Time::ZERO);
        let seek_to = SeekTo::Time { time, track_id: Some(self.track_id) };

        self.format_reader
            .seek(SeekMode::Accurate, seek_to)
            .map_err(|e| DecodeError::Seek(format!("Seek failed: {}", e)))?;

        self.decoder.reset();
        Ok(())
    }

    pub fn info(&self) -> &DecodeInfo {
        &self.info
    }

    pub fn duration_secs(&self) -> f32 {
        self.info.duration_secs
    }
}

/// Extract embedded album art from an audio file and save to local cache directory.
/// Returns the absolute path of the cached image file if artwork is found.
pub fn extract_cover_art_to_cache(path: &Path) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    // Cache directory is constant for the process lifetime; resolve it once.
    static CACHE_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    let cache_dir = CACHE_DIR.get_or_init(|| {
        let mut dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("playtune/covers");
        // Best-effort: create once. Ignore "already exists" errors.
        let _ = std::fs::create_dir_all(&dir);
        Some(dir)
    });
    let cache_dir = cache_dir.as_ref()?;

    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    let hash_id = hasher.finish();

    // Build both candidate filenames once.
    let jpg_path = cache_dir.join(format!("{}.jpg", hash_id));
    if jpg_path.exists() {
        return Some(jpg_path.to_string_lossy().to_string());
    }
    let png_path = cache_dir.join(format!("{}.png", hash_id));
    if png_path.exists() {
        return Some(png_path.to_string_lossy().to_string());
    }

    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let metadata_opts = MetadataOptions::default();
    let format_opts = FormatOptions::default();

    if let Ok(mut format_reader) =
        symphonia::default::get_probe().probe(&hint, mss, format_opts, metadata_opts)
    {
        let mut visual_data = None;
        let mut ext_str = "jpg";

        if let Some(current) = format_reader.metadata().current() {
            let visuals = &current.media.visuals;
            let visual = visuals
                .iter()
                .find(|v| v.usage == Some(StandardVisualKey::FrontCover))
                .or_else(|| visuals.iter().find(|v| v.usage.is_some()))
                .or_else(|| visuals.first());

            if let Some(vis) = visual {
                visual_data = Some(vis.data.to_vec());
                if let Some(ref mt) = vis.media_type {
                    if mt.contains("png") {
                        ext_str = "png";
                    }
                }
            }
        }

        if let Some(data) = visual_data {
            // Downscale the cover to a maximum of 200×200 px before
            // writing to disk. This caps the on-disk cover cache at
            // ~270 KB per album (vs. multi-MB for hi-res album scans),
            // which in turn caps the peak RAM when the CoverLoader
            // decodes the file.
            const MAX_COVER_SIDE: u32 = 200;
            let final_bytes = {
                let decoded = image::ImageReader::new(std::io::Cursor::new(data.as_slice()))
                    .with_guessed_format()
                    .ok()
                    .and_then(|r| r.decode().ok());
                match decoded {
                    Some(img) => {
                        let (w, h) = (img.width(), img.height());
                        let longest = w.max(h);
                        if longest > MAX_COVER_SIDE {
                            let scaled = img.resize(
                                MAX_COVER_SIDE,
                                MAX_COVER_SIDE,
                                image::imageops::FilterType::Lanczos3,
                            );
                            let mut buf = std::io::Cursor::new(Vec::new());
                            let format = if ext_str == "png" {
                                image::ImageFormat::Png
                            } else {
                                image::ImageFormat::Jpeg
                            };
                            match scaled.write_to(&mut buf, format) {
                                Ok(_) => buf.into_inner(),
                                Err(_) => data, // fallback: raw bytes
                            }
                        } else {
                            data
                        }
                    }
                    None => data, // fallback: raw bytes
                }
            };
            let out_path = cache_dir.join(format!("{}.{}", hash_id, ext_str));
            if std::fs::write(&out_path, &final_bytes).is_ok() {
                return Some(out_path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Extract title, artist, album, duration_secs, and duration_str from an audio file.
pub fn extract_track_metadata(path: &Path) -> (String, String, String, f64, String) {
    let default_title =
        path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown Track").to_string();
    let mut title = default_title.clone();
    let mut artist = "Unknown Artist".to_string();
    let mut album = "Unknown Album".to_string();
    let mut duration_secs = 0.0;

    if let Ok(file) = File::open(path) {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let metadata_opts = MetadataOptions::default();
        let format_opts = FormatOptions::default();

        if let Ok(mut format_reader) =
            symphonia::default::get_probe().probe(&hint, mss, format_opts, metadata_opts)
        {
            if let Some(track) = format_reader.tracks().first() {
                if let Some(tb) = track.time_base {
                    if let Some(n_frames) = track.num_frames {
                        if let Some(time) = tb.calc_time(Timestamp::new(n_frames as i64)) {
                            duration_secs = time.as_secs_f64();
                        }
                    }
                }
            }

            if let Some(current) = format_reader.metadata().current() {
                for tag in &current.media.tags {
                    if let Some(std) = &tag.std {
                        match std {
                            StandardTag::TrackTitle(val) if !val.is_empty() => {
                                title = val.to_string();
                            }
                            StandardTag::Artist(val) if !val.is_empty() => {
                                artist = val.to_string();
                            }
                            StandardTag::Album(val) if !val.is_empty() => {
                                album = val.to_string();
                            }
                            _ => {}
                        }
                    } else {
                        let key_str = tag.raw.key.to_lowercase();
                        let val_str = tag.raw.value.to_string();
                        if (key_str.contains("title") || key_str == "tracktitle")
                            && !val_str.is_empty()
                        {
                            title = val_str;
                        } else if key_str.contains("artist") && !val_str.is_empty() {
                            artist = val_str;
                        } else if key_str.contains("album") && !val_str.is_empty() {
                            album = val_str;
                        }
                    }
                }
            }
        }
    }

    let duration_str = if duration_secs > 0.0 {
        format!("{}:{:02}", (duration_secs as i32) / 60, (duration_secs as i32) % 60)
    } else {
        "0:00".to_string()
    };

    (title, artist, album, duration_secs, duration_str)
}

/// Extract ReplayGain / EBU R128 loudness metadata from file tags.
pub fn extract_loudness_metadata(path: &Path) -> crate::dsp::loudness::LoudnessMetadata {
    use crate::dsp::loudness::LoudnessMetadata;

    let mut meta = LoudnessMetadata::default();

    let parse_f32 = |s: &str| -> Option<f32> {
        // Tags often look like "-6.34 dB" — strip non-numeric prefix/suffix.
        let trimmed: String = s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        trimmed.parse::<f32>().ok().filter(|v| v.is_finite())
    };

    // R128 tag values are integer LUFS × 100 (per the EBU R128 tag spec).
    // Some encoders write the value as a plain float LUFS string; we detect
    // both forms by attempting the integer-÷-100 conversion first.
    let parse_r128 = |s: &str| -> Option<f32> {
        let trimmed: String = s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
            .collect();
        if let Ok(v) = trimmed.parse::<f32>() {
            if v.is_finite() {
                // Heuristic: if |v| > 200 it's almost certainly the encoded
                // integer form (a typical track is -23 LUFS = -2300 encoded).
                // Otherwise treat it as a plain LUFS value.
                if v.abs() > 200.0 {
                    return Some(v / 100.0);
                }
                return Some(v);
            }
        }
        None
    };

    if let Ok(file) = File::open(path) {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let metadata_opts = MetadataOptions::default();
        let format_opts = FormatOptions::default();

        if let Ok(mut format_reader) =
            symphonia::default::get_probe().probe(&hint, mss, format_opts, metadata_opts)
        {
            if let Some(current) = format_reader.metadata().current() {
                for tag in &current.media.tags {
                    if let Some(std) = &tag.std {
                        match std {
                            StandardTag::ReplayGainTrackGain(v) => {
                                meta.replaygain_track_db = parse_f32(v);
                            }
                            StandardTag::ReplayGainAlbumGain(v) => {
                                meta.replaygain_album_db = parse_f32(v);
                            }
                            StandardTag::ReplayGainTrackPeak(v) => {
                                meta.replaygain_track_peak = parse_f32(v);
                            }
                            StandardTag::ReplayGainAlbumPeak(v) => {
                                meta.replaygain_album_peak = parse_f32(v);
                            }
                            _ => {}
                        }
                    }
                    let key = tag.raw.key.to_lowercase();
                    let value = tag.raw.value.to_string();
                    if value.is_empty() {
                        continue;
                    }
                    if key == "replaygain_track_gain" && meta.replaygain_track_db.is_none() {
                        meta.replaygain_track_db = parse_f32(&value);
                    } else if key == "replaygain_album_gain" && meta.replaygain_album_db.is_none() {
                        meta.replaygain_album_db = parse_f32(&value);
                    } else if key == "replaygain_track_peak" && meta.replaygain_track_peak.is_none()
                    {
                        meta.replaygain_track_peak = parse_f32(&value);
                    } else if key == "replaygain_album_peak" && meta.replaygain_album_peak.is_none()
                    {
                        meta.replaygain_album_peak = parse_f32(&value);
                    } else if key == "r128_track_gain" {
                        meta.ebu_r128_loudness = parse_r128(&value);
                    } else if key == "r128_album_gain" {
                        // Reuse the same field — AlbumReplayGain mode reads
                        // replaygain_album_db, but if only R128 tags are
                        // present we treat them as the track loudness.
                        if meta.ebu_r128_loudness.is_none() {
                            meta.ebu_r128_loudness = parse_r128(&value);
                        }
                    }
                }
            }
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_info() {
        let info = DecodeInfo {
            sample_rate: 44100,
            channels: 2,
            duration_secs: 180.0,
            codec: "mp3".to_string(),
            bitrate_kbps: Some(320),
        };
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
    }
}

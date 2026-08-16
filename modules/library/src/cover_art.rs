//! Cover art extraction and MIME detection.

use std::path::Path;

use super::{LibraryError, LibraryManager};

/// Cover art data extracted from an audio file's embedded metadata.
pub struct CoverArtData {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub data_hash: String,
    pub width: i32,
    pub height: i32,
}

impl LibraryManager {
    pub(crate) fn extract_visual_from_revision(
        revision: &symphonia::core::meta::MetadataRevision,
    ) -> Option<CoverArtData> {
        use symphonia::core::meta::StandardVisualKey;

        let visuals = &revision.media.visuals;
        let visual = visuals
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover))
            .or_else(|| visuals.iter().find(|v| v.usage.is_some()))
            .or_else(|| visuals.first())?;

        let data = &visual.data;
        const MAX_COVER_ART_BYTES: usize = 2 * 1024 * 1024;
        if data.len() > MAX_COVER_ART_BYTES {
            log::warn!(
                "Skipping cover art: {} bytes exceeds {} byte limit",
                data.len(),
                MAX_COVER_ART_BYTES
            );
            return None;
        }
        let mime_type = visual
            .media_type
            .as_deref()
            .filter(|m| !m.is_empty())
            .map(|m| m.to_string())
            .unwrap_or_else(|| detect_image_mime(data));

        // Decode the source image. If the longest side exceeds 500 px,
        // downscale it before hashing + persisting. This caps the on-
        // disk cover cache at ~500×500×3 ≈ 750 KB per album (vs. the
        // previous unbounded size that could be several MB for hi-res
        // album scans). The 500 px target is comfortably above the
        // largest display size used by the UI (the Albums grid card at
        // 150×150, the Now-Playing card at 140×140) so visible quality
        // is unchanged.
        const MAX_COVER_SIDE: u32 = 500;

        let decoded = image::ImageReader::new(std::io::Cursor::new(data.as_ref()))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.decode().ok());
        let (final_bytes, width, height) = match decoded {
            Some(img) => {
                let (w, h) = (img.width(), img.height());
                let longest = w.max(h);
                if longest > MAX_COVER_SIDE {
                    // Downscale with a Lanczos filter for quality.
                    let scaled = img.resize(
                        MAX_COVER_SIDE,
                        MAX_COVER_SIDE,
                        image::imageops::FilterType::Lanczos3,
                    );
                    // Re-encode to JPEG (or PNG if the source had alpha)
                    // to keep the byte size small.
                    let mut buf = std::io::Cursor::new(Vec::new());
                    let format = if mime_type.contains("png") {
                        image::ImageFormat::Png
                    } else {
                        image::ImageFormat::Jpeg
                    };
                    if scaled.write_to(&mut buf, format).is_err() {
                        // Fallback: keep original bytes if re-encode fails.
                        (data.to_vec(), w as i32, h as i32)
                    } else {
                        let new_bytes = buf.into_inner();
                        (new_bytes, MAX_COVER_SIDE as i32, MAX_COVER_SIDE as i32)
                    }
                } else {
                    (data.to_vec(), w as i32, h as i32)
                }
            }
            None => {
                log::warn!("Cover art bytes failed to decode — skipping");
                return None;
            }
        };

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&final_bytes);
        let data_hash = format!("{:x}", hasher.finalize());
        Some(CoverArtData { data: final_bytes, mime_type, data_hash, width, height })
    }

    pub fn extract_cover_art(
        &self,
        path: &Path,
        track_id: i64,
    ) -> Result<Option<i64>, LibraryError> {
        let (_, _, _, _, cover) = Self::probe_file(path)
            .ok_or_else(|| LibraryError::Other(format!("Probe failed for {}", path.display())))?;

        let art = match cover {
            Some(a) => a,
            None => return Ok(None),
        };

        let album_id = self.db.get_track(track_id).ok().flatten().and_then(|t| {
            // TrackRecord.album is String (not Option); treat empty as None.
            if t.album.is_empty() {
                return None;
            }
            // TrackRecord has no album_artist field; pass None for now.
            // A future schema migration can add album_artist to TrackRecord.
            self.db.get_album_id(&t.album, None).ok().flatten()
        });

        let id = self.db.insert_cover_art(
            album_id,
            Some(track_id),
            None,
            Some(&art.data),
            Some(&art.data_hash),
            art.width,
            art.height,
            &art.mime_type,
        )?;

        log::debug!(
            "Extracted cover art for track {} ({} bytes, {}×{})",
            track_id,
            art.data.len(),
            art.width,
            art.height
        );
        Ok(Some(id))
    }
}

pub fn detect_image_mime(data: &[u8]) -> String {
    if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
        return "image/jpeg".to_string();
    }
    if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        return "image/png".to_string();
    }
    if data.len() >= 4 && data[0..4] == [0x47, 0x49, 0x46, 0x38] {
        return "image/gif".to_string();
    }
    if data.len() >= 12
        && data[0..4] == [0x52, 0x49, 0x46, 0x46]
        && data[8..12] == [0x57, 0x45, 0x42, 0x50]
    {
        return "image/webp".to_string();
    }
    if data.len() >= 2 && data[0..2] == [0x42, 0x4D] {
        return "image/bmp".to_string();
    }
    "application/octet-stream".to_string()
}

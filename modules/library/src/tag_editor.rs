//! Metadata Tag Editor using `lofty` for ID3v2, Vorbis Comments, and MP4/WAV tags.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use db::PlayTuneDb as Database;
use db::TrackRecord;
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{Accessor, Tag, TagExt};
use log::{debug, info};

use crate::LibraryError;

#[derive(Debug, Clone)]
pub struct TagEditRequest {
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    /// `Some(path)` where path is not empty -> set/replace cover image.
    /// `Some(path)` where path is empty -> remove cover image.
    /// `None` -> leave cover image unchanged.
    pub cover_image_path: Option<String>,
}

/// Update metadata tags on disk using `lofty`, then synchronize the database (`tracks` and `cover_art`)
/// and cover image cache.
pub fn update_track_tags(db: &Database, req: TagEditRequest) -> Result<TrackRecord, LibraryError> {
    let track = match db.get_track(req.track_id)? {
        Some(t) => t,
        None => {
            return Err(LibraryError::Other(format!(
                "Track ID {} not found in database",
                req.track_id
            )))
        }
    };

    let path = Path::new(track.path.as_ref());
    if !path.exists() {
        return Err(LibraryError::Other(format!(
            "Audio file does not exist on disk: {}",
            track.path
        )));
    }

    // Open and parse tags with lofty
    let mut tagged_file = match lofty::read_from_path(path) {
        Ok(f) => f,
        Err(e) => {
            return Err(LibraryError::Other(format!(
                "Failed to read tags from {}: {}",
                track.path, e
            )))
        }
    };

    // Ensure we have a primary tag to edit
    let tag = match tagged_file.primary_tag_mut() {
        Some(t) => t,
        None => {
            let tag_type = tagged_file.primary_tag_type();
            tagged_file.insert_tag(Tag::new(tag_type));
            match tagged_file.primary_tag_mut() {
                Some(t) => t,
                None => {
                    return Err(LibraryError::Other(format!(
                    "Failed to create primary tag for {} (lofty returned None after insert_tag)",
                    track.path
                )))
                }
            }
        }
    };

    // Update text attributes via Accessor trait
    tag.set_title(req.title.clone());
    tag.set_artist(req.artist.clone());
    tag.set_album(req.album.clone());

    if let Some(ref aa) = req.album_artist {
        if aa.is_empty() {
            tag.remove_key(&lofty::tag::ItemKey::AlbumArtist);
        } else {
            tag.insert_text(lofty::tag::ItemKey::AlbumArtist, aa.clone());
        }
    }

    if let Some(ref genre) = req.genre {
        if genre.is_empty() {
            tag.remove_genre();
        } else {
            tag.set_genre(genre.clone());
        }
    }

    if let Some(year) = req.year {
        if year > 0 {
            tag.set_year(year);
        } else {
            tag.remove_year();
        }
    }

    if let Some(track_num) = req.track_number {
        if track_num > 0 {
            tag.set_track(track_num);
        } else {
            tag.remove_track();
        }
    }

    if let Some(disc_num) = req.disc_number {
        if disc_num > 0 {
            tag.set_disk(disc_num);
        } else {
            tag.remove_disk();
        }
    }

    // Handle cover art changes if requested
    let mut cover_art_changed = false;
    if let Some(ref cover_path) = req.cover_image_path {
        cover_art_changed = true;
        if cover_path.is_empty() {
            // Clear existing pictures
            tag.remove_picture_type(lofty::picture::PictureType::CoverFront);
            debug!("Removed cover art from {}", track.path);
        } else {
            let img_data = std::fs::read(cover_path).map_err(|e| {
                LibraryError::Other(format!("Failed to read image {}: {}", cover_path, e))
            })?;
            let mime = crate::detect_image_mime(&img_data);
            let mime_type = lofty::picture::MimeType::from_str(&mime);
            let picture = lofty::picture::Picture::new_unchecked(
                lofty::picture::PictureType::CoverFront,
                Some(mime_type),
                None,
                img_data,
            );
            tag.set_picture(0, picture);
            debug!("Updated cover art on {} from {}", track.path, cover_path);
        }
    }

    // Save modified tag back to disk
    if let Err(e) = tag.save_to_path(path, WriteOptions::default()) {
        return Err(LibraryError::Other(format!("Failed to save tags to {}: {}", track.path, e)));
    }

    info!("Successfully wrote metadata tags to {}", track.path);

    // Get the updated file modification time
    let new_mtime = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(track.file_modified);

    // If cover art was changed or cleared, clean out any stale cover cache files
    if cover_art_changed {
        clean_cover_cache(path, track.id);
    }

    // Update `tracks` table in database
    let duration_secs = track.duration_secs;
    let duration_str = track.duration_str.clone();
    let folder_id = track.folder_id;

    db.add_or_update_track_with_lyrics(
        &track.path,
        if req.title.is_empty() { "Unknown" } else { &req.title },
        if req.artist.is_empty() { "Unknown" } else { &req.artist },
        if req.album.is_empty() { "Unknown" } else { &req.album },
        duration_secs,
        &duration_str,
        folder_id,
        new_mtime,
        None,
        None,
        req.track_number.map(|tn| tn as i32),
    )?;

    // If cover art changed, update `cover_art` table and extract fresh cache
    if cover_art_changed {
        // Delete old DB cover entries for this track
        let _ = db.delete_cover_art_by_track(track.id);

        if let Some(ref cover_path) = req.cover_image_path {
            if !cover_path.is_empty() {
                // Re-probe and store new cover art in DB and cache
                let _ = engine::extract_cover_art_to_cache(path);
                if let Ok(img_data) = std::fs::read(cover_path) {
                    let mime = crate::detect_image_mime(&img_data);
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(&img_data);
                    let hash_str = format!("{:x}", hasher.finalize());
                    let dims = image::ImageReader::new(std::io::Cursor::new(&img_data))
                        .with_guessed_format()
                        .ok()
                        .and_then(|r| r.into_dimensions().ok())
                        .unwrap_or((600, 600));

                    let album_id = db.get_album_id(&req.album, None).ok().flatten();
                    let _ = db.insert_cover_art(
                        album_id,
                        Some(track.id),
                        folder_id,
                        Some(&img_data),
                        Some(&hash_str),
                        dims.0 as i32,
                        dims.1 as i32,
                        &mime,
                    );
                }
            }
        }
    }

    // Return the refreshed track record
    let updated_track = db.get_track(req.track_id)?.ok_or_else(|| {
        LibraryError::Other("Failed to reload track from DB after update".to_string())
    })?;

    Ok(updated_track)
}

fn clean_cover_cache(path: &Path, track_id: i64) {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    let hash_id = hasher.finish();

    if let Some(data_local) = dirs::data_local_dir() {
        let cache_dir = data_local.join("playtune/covers");
        let _ = std::fs::remove_file(cache_dir.join(format!("{}.jpg", hash_id)));
        let _ = std::fs::remove_file(cache_dir.join(format!("{}.png", hash_id)));
        let _ = std::fs::remove_file(cache_dir.join(format!("{}.webp", hash_id)));
    }
    let _ = track_id;
}

pub fn get_track_tags(db: &Database, track_id: i64) -> Result<TagEditRequest, LibraryError> {
    let track = match db.get_track(track_id)? {
        Some(t) => t,
        None => {
            return Err(LibraryError::Other(format!("Track ID {} not found in database", track_id)))
        }
    };

    let path = Path::new(track.path.as_ref());
    let mut title = track.title.to_string();
    let mut artist = track.artist.to_string();
    let mut album = track.album.to_string();
    let mut album_artist = None;
    let mut genre = None;
    let mut year = None;
    let mut track_number = None;
    let mut disc_number = None;

    if let Ok(tagged_file) = lofty::read_from_path(path) {
        if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            if let Some(t) = tag.title() {
                if !t.is_empty() {
                    title = t.to_string();
                }
            }
            if let Some(a) = tag.artist() {
                if !a.is_empty() {
                    artist = a.to_string();
                }
            }
            if let Some(al) = tag.album() {
                if !al.is_empty() {
                    album = al.to_string();
                }
            }

            if let Some(aa) = tag.get_string(&lofty::tag::ItemKey::AlbumArtist) {
                if !aa.is_empty() {
                    album_artist = Some(aa.to_string());
                }
            } else if let Some(aa) = tag.get_string(&lofty::tag::ItemKey::TrackArtist) {
                if !aa.is_empty() {
                    album_artist = Some(aa.to_string());
                }
            }
            if let Some(g) = tag.genre() {
                if !g.is_empty() {
                    genre = Some(g.to_string());
                }
            }
            if let Some(y) = tag.year() {
                if y > 0 {
                    year = Some(y);
                }
            }
            if let Some(tn) = tag.track() {
                if tn > 0 {
                    track_number = Some(tn);
                }
            }
            if let Some(dn) = tag.disk() {
                if dn > 0 {
                    disc_number = Some(dn);
                }
            }
        }
    } else if let Some(file_tags) = crate::LibraryManager::read_file_tags(path) {
        if let Some(t) = file_tags.title {
            if !t.is_empty() {
                title = t;
            }
        }
        if let Some(a) = file_tags.artist {
            if !a.is_empty() {
                artist = a;
            }
        }
        if let Some(al) = file_tags.album {
            if !al.is_empty() {
                album = al;
            }
        }
        album_artist = file_tags.album_artist;
        genre = file_tags.genre;
        if let Some(y) = file_tags.year {
            if y > 0 {
                year = Some(y as u32);
            }
        }
        if let Some(tn) = file_tags.track_number {
            if tn > 0 {
                track_number = Some(tn as u32);
            }
        }
        if let Some(dn) = file_tags.disc_number {
            if dn > 0 {
                disc_number = Some(dn as u32);
            }
        }
    }

    let cover_path = match dirs::cache_dir() {
        Some(mut p) => {
            p.push("playtune");
            p.push("covers");
            let jpg = p.join(format!("{}.jpg", track.id));
            let png = p.join(format!("{}.png", track.id));
            let webp = p.join(format!("{}.webp", track.id));
            if jpg.exists() {
                Some(jpg.to_string_lossy().to_string())
            } else if png.exists() {
                Some(png.to_string_lossy().to_string())
            } else if webp.exists() {
                Some(webp.to_string_lossy().to_string())
            } else {
                None
            }
        }
        None => None,
    };

    Ok(TagEditRequest {
        track_id,
        title,
        artist,
        album,
        album_artist,
        genre,
        year,
        track_number,
        disc_number,
        cover_image_path: cover_path,
    })
}

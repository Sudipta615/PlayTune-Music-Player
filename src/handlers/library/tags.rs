use crate::app_state::{invalidate_all_views, invalidate_cover_cache, GLOBAL_DB};
use crate::bridge;

#[no_mangle]
pub extern "C" fn playtune_get_track_lyrics(track_id: std::ffi::c_int) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(db) = GLOBAL_DB.get() {
            if let Ok(Some(track)) = db.get_track(track_id as i64) {
                bridge::update_track_lyrics(
                    track.id as i32,
                    track.lyrics_synced.as_deref(),
                    track.lyrics_unsynced.as_deref(),
                );
            }
        }
    }));
    if result.is_err() {
        log::error!("panic inside playtune_get_track_lyrics");
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_update_track_tags(
    req: *const bridge::FfiTagEditRequest,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if req.is_null() {
            return 0;
        }
        let req = unsafe { &*req };
        let db = match GLOBAL_DB.get() {
            Some(db) => db,
            None => {
                log::error!("Database not initialized when updating tags");
                return 0;
            }
        };

        let title_str = unsafe {
            if req.title.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.title).to_string_lossy().to_string()
            }
        };
        let artist_str = unsafe {
            if req.artist.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.artist).to_string_lossy().to_string()
            }
        };
        let album_str = unsafe {
            if req.album.is_null() {
                "".to_string()
            } else {
                std::ffi::CStr::from_ptr(req.album).to_string_lossy().to_string()
            }
        };
        let album_artist_str = unsafe {
            if req.album_artist.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.album_artist).to_string_lossy().to_string())
            }
        };
        let genre_str = unsafe {
            if req.genre.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.genre).to_string_lossy().to_string())
            }
        };
        let cover_path_opt = unsafe {
            if req.cover_image_path.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(req.cover_image_path).to_string_lossy().to_string())
            }
        };

        let tag_req = library::TagEditRequest {
            track_id: req.track_id as i64,
            title: title_str,
            artist: artist_str,
            album: album_str,
            album_artist: album_artist_str,
            genre: genre_str,
            year: Some(req.year),
            track_number: Some(req.track_number),
            disc_number: Some(req.disc_number),
            cover_image_path: cover_path_opt,
        };

        match library::update_track_tags(db, tag_req) {
            Ok(updated_track) => {
                log::info!("Successfully updated tags for track ID {}", updated_track.id);
                invalidate_cover_cache(&updated_track.path);
                invalidate_all_views();
                let cover_path =
                    crate::app_state::cached_cover_path(&updated_track.path).unwrap_or_default();

                bridge::update_track_metadata(
                    updated_track.id as i32,
                    &updated_track.title,
                    &updated_track.artist,
                    &updated_track.album,
                    &updated_track.duration_str,
                    &cover_path,
                );
                1
            }
            Err(e) => {
                log::error!("Failed to update track tags: {}", e);
                0
            }
        }
    }));
    match result {
        Ok(ret) => ret,
        Err(_) => {
            log::error!("panic inside playtune_update_track_tags");
            0
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn playtune_get_track_tags(
    track_id: std::ffi::c_int,
    title_buf: *mut std::ffi::c_char,
    title_len: std::ffi::c_int,
    artist_buf: *mut std::ffi::c_char,
    artist_len: std::ffi::c_int,
    album_buf: *mut std::ffi::c_char,
    album_len: std::ffi::c_int,
    album_artist_buf: *mut std::ffi::c_char,
    album_artist_len: std::ffi::c_int,
    genre_buf: *mut std::ffi::c_char,
    genre_len: std::ffi::c_int,
    year_out: *mut std::ffi::c_uint,
    track_num_out: *mut std::ffi::c_uint,
    disc_num_out: *mut std::ffi::c_uint,
    cover_buf: *mut std::ffi::c_char,
    cover_len: std::ffi::c_int,
) -> std::ffi::c_int {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let db = match crate::GLOBAL_DB.get() {
            Some(db) => db,
            None => return 0,
        };

        let tag_req = match library::get_track_tags(db, track_id as i64) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("playtune_get_track_tags failed for {}: {}", track_id, e);
                return 0;
            }
        };

        unsafe {
            let copy_str = |s: &str, buf: *mut std::ffi::c_char, max_len: std::ffi::c_int| {
                if !buf.is_null() && max_len > 0 {
                    let bytes = s.as_bytes();
                    let to_copy = bytes.len().min((max_len - 1) as usize);
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr() as *const std::ffi::c_char,
                        buf,
                        to_copy,
                    );
                    *buf.add(to_copy) = 0;
                }
            };

            copy_str(&tag_req.title, title_buf, title_len);
            copy_str(&tag_req.artist, artist_buf, artist_len);
            copy_str(&tag_req.album, album_buf, album_len);
            copy_str(
                tag_req.album_artist.as_deref().unwrap_or(""),
                album_artist_buf,
                album_artist_len,
            );
            copy_str(tag_req.genre.as_deref().unwrap_or(""), genre_buf, genre_len);

            if !year_out.is_null() {
                *year_out = tag_req.year.unwrap_or(0);
            }
            if !track_num_out.is_null() {
                *track_num_out = tag_req.track_number.unwrap_or(0);
            }
            if !disc_num_out.is_null() {
                *disc_num_out = tag_req.disc_number.unwrap_or(0);
            }

            copy_str(tag_req.cover_image_path.as_deref().unwrap_or(""), cover_buf, cover_len);
        }

        1
    }));

    result.unwrap_or(0)
}

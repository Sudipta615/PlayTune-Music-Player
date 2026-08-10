pub mod browsing;
pub mod folders;
pub mod import;
pub mod loudness;
pub mod playlists;
pub mod search;
pub mod tags;

pub use browsing::*;
pub use folders::*;
pub use import::*;
pub use loudness::*;
pub use playlists::*;
pub use search::*;
pub use tags::*;

#[allow(clippy::missing_transmute_annotations)]
#[used]
static _EXPORTED_SYMBOLS: [unsafe extern "C" fn(); 8] = [
    unsafe {
        std::mem::transmute(tags::playtune_get_track_lyrics as extern "C" fn(std::ffi::c_int))
    },
    unsafe {
        std::mem::transmute(
            tags::playtune_update_track_tags
                as extern "C" fn(*const crate::bridge::FfiTagEditRequest) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            tags::playtune_get_track_tags
                as extern "C" fn(
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_uint,
                    *mut std::ffi::c_char,
                    std::ffi::c_int,
                ) -> std::ffi::c_int,
        )
    },
    unsafe { std::mem::transmute(loudness::playtune_cancel_loudness_scan as extern "C" fn()) },
    unsafe {
        std::mem::transmute(
            loudness::playtune_start_loudness_scan
                as extern "C" fn(*const std::ffi::c_int, std::ffi::c_int),
        )
    },
    unsafe {
        std::mem::transmute(
            loudness::playtune_write_loudness_results
                as extern "C" fn(
                    *const crate::bridge::FfiLoudnessWriteItem,
                    std::ffi::c_int,
                ) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            playlists::playtune_import_m3u
                as extern "C" fn(
                    *const std::ffi::c_char,
                    *const std::ffi::c_char,
                ) -> std::ffi::c_int,
        )
    },
    unsafe {
        std::mem::transmute(
            playlists::playtune_export_m3u
                as extern "C" fn(std::ffi::c_int, *const std::ffi::c_char) -> std::ffi::c_int,
        )
    },
];

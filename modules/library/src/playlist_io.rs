//! M3U / M3U8 playlist import & export.
//!
//! The format is the de-facto universal playlist format supported by
//! virtually every music player (AIMP, Foobar2000, VLC, mpv, ...). It is
//! a plain-text file with one filepath per line, optionally preceded by
//! `#EXTINF:<duration>,<artist> - <title>` metadata lines. M3U8 is the
//! UTF-8 encoded variant (functionally identical for our purposes — we
//! always read and write as UTF-8).
//!
//! ## Import
//! Walks each entry, resolves it against the library DB (by exact path
//! match first, then by basename, then by `(title, artist)` heuristic),
//! and returns the list of resolved track ids. Unresolved entries are
//! collected into a "skipped" list so the UI can report them.
//!
//! ## Export
//! Writes the tracks of a given playlist (or any `Vec<TrackRecord>`) to
//! disk, using relative paths when the playlist file lives inside or
//! beneath a music folder (for portability), absolute paths otherwise.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use db::TrackRecord;

/// A single entry parsed from an M3U file, before resolution to a DB track.
#[derive(Debug, Clone)]
pub struct ParsedM3UEntry {
    /// The raw path/URI as written in the file (may be relative, absolute,
    /// or a `file://` URI).
    pub raw_path: String,
    /// Optional `#EXTINF` metadata: duration in seconds (or -1 if unknown)
    /// and the display name (typically "Artist - Title").
    pub duration_secs: Option<f64>,
    pub display_name: Option<String>,
}

/// Result of importing an M3U file.
#[derive(Debug, Clone, Default)]
pub struct M3UImportResult {
    /// DB track ids that were successfully resolved.
    pub resolved_track_ids: Vec<i64>,
    /// Raw entries that could not be matched to any DB track.
    pub skipped_entries: Vec<ParsedM3UEntry>,
}

/// Parse an M3U/M3U8 file from text content. Returns the list of entries
/// in the order they appear in the file.
///
/// The parser is lenient: lines that are not `#EXTM3U`, `#EXTINF`, or
/// paths are silently ignored. Both `\n` and `\r\n` line endings are
/// accepted. `file://` URIs are decoded in-place.
pub fn parse_m3u(text: &str) -> Vec<ParsedM3UEntry> {
    let mut entries = Vec::new();
    let mut pending_duration: Option<f64> = None;
    let mut pending_name: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            // Directive line.
            if let Some(rest) = line.strip_prefix("#EXTINF:") {
                // Format: #EXTINF:<duration>,<display name>
                let (dur_str, name) = match rest.split_once(',') {
                    Some((d, n)) => (d, Some(n.to_string())),
                    None => (rest, None),
                };
                pending_duration = dur_str.trim().parse::<f64>().ok();
                pending_name = name;
            }
            // Other directives (#EXTM3U, #EXTALB, #EXTART, #PLAYLIST, etc.)
            // are recognised and ignored — they don't precede a path entry.
            continue;
        }
        // Path line — decode file:// URI if present.
        let path_str = decode_file_uri(line);
        entries.push(ParsedM3UEntry {
            raw_path: path_str,
            duration_secs: pending_duration.take(),
            display_name: pending_name.take(),
        });
    }
    entries
}

/// Decode a `file://` URI to a plain filesystem path. Non-URI strings are
/// returned unchanged. Percent-encoding is decoded (e.g. `%20` → space).
pub fn decode_file_uri(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("file://") {
        // Strip leading slash on Windows-style paths: file:///C:/foo → C:/foo
        let stripped = rest.trim_start_matches('/');
        let decoded = percent_decode(stripped);
        // Re-add the leading slash for Unix paths (we stripped one too many
        // if the original was file:///home/...).
        if cfg!(windows) && decoded.len() >= 2 && decoded.as_bytes()[1] == b':' {
            decoded
        } else {
            format!("/{}", decoded)
        }
    } else {
        s.to_string()
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Read and parse an M3U file from disk. Returns an error if the file
/// cannot be read.
pub fn read_m3u_file(path: &Path) -> Result<Vec<ParsedM3UEntry>, String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read M3U file {}: {}", path.display(), e))?;
    Ok(parse_m3u(&text))
}

pub fn resolve_entries(
    entries: &[ParsedM3UEntry],
    db: &db::PlayTuneDb,
    playlist_file_dir: Option<&Path>,
    watch_dirs: &[PathBuf],
) -> Result<M3UImportResult, db::DbError> {
    let all_tracks = db.get_all_tracks()?;
    let mut result = M3UImportResult::default();

    for entry in entries {
        let candidate_paths = candidate_paths_for_entry(entry, playlist_file_dir, watch_dirs);
        if let Some(track) = candidate_paths
            .iter()
            .find_map(|p| all_tracks.iter().find(|t| paths_equivalent(&t.path, p)))
        {
            result.resolved_track_ids.push(track.id);
            continue;
        }
        // Basename fallback.
        if let Some(track) = candidate_paths.iter().find_map(|p| {
            let p_str = p.to_string_lossy();
            let basename = path_basename(&p_str)?;
            all_tracks.iter().find(|t| path_basename(&t.path).as_deref() == Some(basename.as_str()))
        }) {
            result.resolved_track_ids.push(track.id);
            continue;
        }
        // EXTINF heuristic.
        if let Some(name) = &entry.display_name {
            if let Some(track) = match_by_display_name(name, &all_tracks) {
                result.resolved_track_ids.push(track.id);
                continue;
            }
        }
        result.skipped_entries.push(entry.clone());
    }

    Ok(result)
}

fn candidate_paths_for_entry(
    entry: &ParsedM3UEntry,
    playlist_file_dir: Option<&Path>,
    watch_dirs: &[PathBuf],
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let raw = PathBuf::from(&entry.raw_path);
    out.push(raw.clone());
    if !raw.is_absolute() {
        if let Some(dir) = playlist_file_dir {
            out.push(dir.join(&raw));
        }
        for wd in watch_dirs {
            out.push(wd.join(&raw));
        }
    }
    out
}

/// Compare two filesystem paths for equivalence, tolerating differences
/// in trailing slashes, redundant `.`/`..` segments, and case on
/// case-insensitive filesystems.
fn paths_equivalent(a: &str, b: &Path) -> bool {
    let pa = Path::new(a);
    let pa_clean = normalise_path(pa);
    let pb_clean = normalise_path(b);
    let a_str = pa_clean.to_string_lossy();
    let b_str = pb_clean.to_string_lossy();
    if cfg!(windows) {
        a_str.eq_ignore_ascii_case(&b_str)
    } else {
        a_str == b_str
    }
}

fn normalise_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in p.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn path_basename(p: &str) -> Option<String> {
    Path::new(p).file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
}

fn match_by_display_name<'a>(name: &str, tracks: &'a [TrackRecord]) -> Option<&'a TrackRecord> {
    // EXTINF names are usually "Artist - Title" but can also be just "Title".
    if let Some((artist, title)) = name.split_once(" - ") {
        let artist = artist.trim().to_lowercase();
        let title = title.trim().to_lowercase();
        if let Some(t) = tracks
            .iter()
            .find(|t| t.artist.to_lowercase() == artist && t.title.to_lowercase() == title)
        {
            return Some(t);
        }
    }
    let lower = name.trim().to_lowercase();
    tracks.iter().find(|t| t.title.to_lowercase() == lower)
}

/// Write an M3U8 file containing the given tracks. Paths are made
/// relative to `playlist_file_path`'s parent directory when possible
/// (i.e. when the track lives beneath that directory); otherwise they
/// are written as absolute paths.
pub fn write_m3u_file(
    playlist_file_path: &Path,
    tracks: &[TrackRecord],
    playlist_name: Option<&str>,
) -> Result<(), String> {
    let parent = playlist_file_path.parent().unwrap_or_else(|| Path::new("."));
    let mut content = String::new();
    content.push_str("#EXTM3U\n");
    if let Some(name) = playlist_name {
        content.push_str(&format!("#PLAYLIST:{}\n", name));
    }
    for t in tracks {
        let display = if !t.artist.is_empty() {
            format!("{} - {}", t.artist, t.title)
        } else {
            t.title.clone()
        };
        let dur = if t.duration_secs > 0.0 { t.duration_secs as i64 } else { -1 };
        content.push_str(&format!("#EXTINF:{},{}\n", dur, display));
        let path_str = path_for_export(&t.path, parent);
        content.push_str(&path_str);
        content.push('\n');
    }

    // Atomic write: write to a .tmp file then rename.
    let tmp_path = playlist_file_path.with_extension("m3u8.tmp");
    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| format!("Failed to create temp file {}: {}", tmp_path.display(), e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write M3U content: {}", e))?;
    file.sync_all().map_err(|e| format!("Failed to sync M3U file: {}", e))?;
    drop(file);
    if let Err(e) = fs::rename(&tmp_path, playlist_file_path) {
        let rename_err = format!(
            "Failed to rename {} → {}: {}",
            tmp_path.display(),
            playlist_file_path.display(),
            e
        );
        // Fall back to copy + remove (cross-device safe).
        match fs::copy(&tmp_path, playlist_file_path) {
            Ok(_) => {
                let _ = fs::remove_file(&tmp_path);
            }
            Err(copy_err) => {
                // Best-effort cleanup of the .tmp file.
                let _ = fs::remove_file(&tmp_path);
                return Err(format!(
                    "{} (and cross-device copy also failed: {})",
                    rename_err, copy_err
                ));
            }
        }
        return Ok(());
    }
    Ok(())
}

/// Convert a track path to the form that should be written to an M3U
/// file. If the track lives beneath `base_dir`, return a relative path;
/// otherwise return the absolute path as-is.
fn path_for_export(track_path: &str, base_dir: &Path) -> String {
    let abs = Path::new(track_path);
    if let Ok(rel) = abs.strip_prefix(base_dir) {
        // Use forward slashes for cross-platform M3U portability.
        rel.to_string_lossy().replace('\\', "/")
    } else {
        track_path.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(id: i64, path: &str, title: &str, artist: &str) -> TrackRecord {
        TrackRecord {
            id,
            path: path.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: String::new(),
            duration_secs: 200.0,
            duration_str: "3:20".to_string(),
            folder_id: None,
            is_favorite: false,
            play_count: 0,
            last_played_at: None,
            file_modified: 0,
            replaygain_track_db: None,
            replaygain_album_db: None,
            replaygain_track_peak: None,
            replaygain_album_peak: None,
            ebu_r128_loudness: None,
            ebu_r128_peak: None,
            lyrics_synced: None,
            lyrics_unsynced: None,
            rating: 0,
            track_number: None,
        }
    }

    #[test]
    fn test_parse_m3u_basic() {
        let text = "#EXTM3U\n#EXTINF:200,Artist A - Song One\n/m/a/1.mp3\n#EXTINF:180,Artist B - Song Two\n/m/a/2.mp3\n";
        let entries = parse_m3u(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].raw_path, "/m/a/1.mp3");
        assert_eq!(entries[0].duration_secs, Some(200.0));
        assert_eq!(entries[0].display_name.as_deref(), Some("Artist A - Song One"));
        assert_eq!(entries[1].raw_path, "/m/a/2.mp3");
    }

    #[test]
    fn test_parse_m3u_file_uri() {
        let text = "#EXTM3U\nfile:///home/user/music/song.mp3\n";
        let entries = parse_m3u(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "/home/user/music/song.mp3");
    }

    #[test]
    fn test_parse_m3u_percent_encoded() {
        let text = "#EXTM3U\nfile:///home/user/My%20Music/song.mp3\n";
        let entries = parse_m3u(text);
        assert_eq!(entries[0].raw_path, "/home/user/My Music/song.mp3");
    }

    #[test]
    fn test_parse_m3u_skip_blank_lines() {
        let text = "#EXTM3U\n\n\n#EXTINF:-1,Unknown\n/m/a/1.mp3\n\n";
        let entries = parse_m3u(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].duration_secs, Some(-1.0));
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = std::env::temp_dir().join("playtune_m3u_test.m3u8");
        let tracks = vec![
            make_track(1, "/home/user/song1.mp3", "Song One", "Artist A"),
            make_track(2, "/home/user/song2.mp3", "Song Two", "Artist B"),
        ];
        write_m3u_file(&tmp, &tracks, Some("My Playlist")).unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();
        assert!(text.starts_with("#EXTM3U\n"));
        assert!(text.contains("#PLAYLIST:My Playlist"));
        assert!(text.contains("#EXTINF:200,Artist A - Song One"));
        assert!(text.contains("/home/user/song1.mp3"));

        let entries = parse_m3u(&text);
        assert_eq!(entries.len(), 2);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn test_relative_path_export() {
        let tmp = std::env::temp_dir().join("playtune_m3u_rel_test/playlist.m3u8");
        std::fs::create_dir_all(tmp.parent().unwrap()).unwrap();
        // Track beneath the playlist's parent — should be relative.
        let track_path =
            tmp.parent().unwrap().join("subdir").join("song.mp3").to_string_lossy().to_string();
        let tracks = vec![make_track(1, &track_path, "Song", "Artist")];
        write_m3u_file(&tmp, &tracks, None).unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();
        // The path should be relative — no leading slash, no drive letter.
        assert!(text.contains("subdir/song.mp3"));
        std::fs::remove_file(&tmp).ok();
        std::fs::remove_dir(tmp.parent().unwrap()).ok();
    }

    #[test]
    fn test_resolve_entries_exact_path() {
        let db = db::PlayTuneDb::open_in_memory().unwrap();
        db.add_or_update_track(
            "/m/a/1.mp3",
            "Song One",
            "Artist A",
            "Album",
            200.0,
            "3:20",
            None,
            0,
        )
        .unwrap();
        db.add_or_update_track(
            "/m/a/2.mp3",
            "Song Two",
            "Artist B",
            "Album",
            180.0,
            "3:00",
            None,
            0,
        )
        .unwrap();
        let entries = vec![
            ParsedM3UEntry {
                raw_path: "/m/a/1.mp3".to_string(),
                duration_secs: None,
                display_name: None,
            },
            ParsedM3UEntry {
                raw_path: "/m/a/2.mp3".to_string(),
                duration_secs: None,
                display_name: None,
            },
            ParsedM3UEntry {
                raw_path: "/nonexistent.mp3".to_string(),
                duration_secs: None,
                display_name: None,
            },
        ];
        let result = resolve_entries(&entries, &db, None, &[]).unwrap();
        assert_eq!(result.resolved_track_ids.len(), 2);
        assert_eq!(result.skipped_entries.len(), 1);
    }

    #[test]
    fn test_resolve_entries_extinf_heuristic() {
        let db = db::PlayTuneDb::open_in_memory().unwrap();
        db.add_or_update_track(
            "/m/a/1.mp3",
            "Song One",
            "Artist A",
            "Album",
            200.0,
            "3:20",
            None,
            0,
        )
        .unwrap();
        let entries = vec![ParsedM3UEntry {
            raw_path: "/somewhere/else.mp3".to_string(),
            duration_secs: None,
            display_name: Some("Artist A - Song One".to_string()),
        }];
        let result = resolve_entries(&entries, &db, None, &[]).unwrap();
        assert_eq!(result.resolved_track_ids.len(), 1);
        assert_eq!(result.resolved_track_ids[0], 1);
    }
}

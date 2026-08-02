# 📜 PlayTune / TuneCraft — Change Log

All notable changes to **PlayTune (TuneCraft)** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project strictly adheres to the **3-Way (x.y.z) Semantic Versioning** system.

---

## 📌 Versioning Rules & Developer Instructions

All developers and contributors **MUST** follow the standard 3-way (`x.y.z`) Semantic Versioning scheme:

```
  MAJOR . MINOR . PATCH  (e.g., 0.9.0)
    ^       ^       ^
    │       │       └── Bug fixes, performance tweaks, & internal refactoring (no API/schema breaks)
    │       └────────── New features or substantial UI/DSP additions (backward-compatible)
    └────────────────── Breaking changes, schema revamps, or incompatible FFI/API updates
```

### Version Bump Guidelines
1. **`PATCH` (`x.y.Z`)**:
   - Incremented for bug fixes, hotfixes, memory leak resolutions, optimization patches, or non-breaking internal refactoring.
   - Example: `0.3.0` → `0.3.1` (fixes resampler truncation or track borrow issues).

2. **`MINOR` (`x.Y.0`)**:
   - Incremented when adding significant new features, new UI dialogs, additional DSP modules, or expanded codec support in a backward-compatible manner.
   - Resets the `PATCH` version to `0`.
   - Example: `0.8.0` → `0.9.0` (completing core pre-release feature suite & changelog integration).

3. **`MAJOR` (`X.0.0`)**:
   - Incremented for major milestone releases (e.g., `1.0.0` Production Release), breaking FFI bridge modifications, or non-backward-compatible database/config schema overhauls.
   - Resets both `MINOR` and `PATCH` versions to `0`.

### Workspace Versioning Discipline
- When updating the app version, update `version` under both `[package]` and `[workspace.package]` in `Cargo.toml`. All member crates (`modules/engine`, `modules/gui`, `modules/db`, `modules/library`, `modules/config`, `modules/platform`, `modules/analysis`) inherit `version.workspace = true`.
- Run `cargo check` or `cargo build` to ensure `Cargo.lock` is synchronized.
- Every release **MUST** add a corresponding entry in this `CHANGELOG.md` file under the appropriate version section before tagging or releasing.

---

## 🗓️ Version History

### [1.0.6] — 2026-08-02 — High-Contrast Duration Labels & Styling

#### Summary
Enhanced the visual contrast and legibility of player card track duration labels on dynamic backdrop gradients.

#### Added
- **Glassmorphism Duration Pill Badges**: Styled `m_timeElapsed` and `m_timeTotal` labels in `NowPlayingCard` with a semi-transparent dark pill background (`rgba(10, 12, 20, 0.55)`), white border stroke (`rgba(255, 255, 255, 0.18)`), high-contrast bold white text (`#FFFFFF`, `font-weight: 600`), and a drop shadow effect for maximum readability on any background gradient.

---

### [1.0.5] — 2026-08-02 — Persistent Shuffle Queue Fix

#### Summary
Resolved persistent shuffle queue playback synchronization across UI components and queue reordering.

#### Added
- **Persistent Shuffle Queue (`SHUFFLE_ORDER`)**: Added global `SHUFFLE_ORDER` and `SHUFFLE_POS` state in `app_state.rs` with `sync_shuffle_order()` helper to maintain a single, consistent shuffled index sequence across UI rendering and playback transitions.

#### Fixed
- **Shuffle Queue Playback Mismatch**: Fixed a bug where toggling shuffle displayed a shuffled track sequence in the Up Next queue panel, but pressing Next or auto-advancing on track completion re-sampled a new pseudo-random track instead of playing the displayed queue item.
- **Queue Drag-and-Drop Reordering**: Updated `rust_reorder_queue` to reorder elements within `SHUFFLE_ORDER` when shuffle is enabled, preserving user queue reordering.
- **Unified Ticker Auto-Advance**: Standardized automatic track transition on track completion to invoke `rust_next()`.

---

### [1.0.4] — 2026-08-02 — Performance, UI & Responsiveness Optimization Release

#### Summary
Comprehensive performance overhaul addressing high CPU usage during tab/mode switching and fast scrolling, multi-second UI freezes during large folder library imports, seekbar click-to-seek functionality, and spinbox stepper button visual contrast across dark theme controls.

#### Added
- **Direct Click-to-Seek (`ClickableSlider`)**: Created a custom `ClickableSlider` subclassing `QSlider` in `custom_widgets.h` that maps click coordinates directly to slider range values on `mousePressEvent`. Applied to `NowPlayingCard` seekbar and `QueueWidget` volume slider.
- **Waveform Visualizer Click-to-Seek**: Integrated direct click-to-seek functionality on `WaveformVisualizer` bars in `NowPlayingCard`.
- **High-Contrast SpinBox Stepper Buttons**: Added custom QSS styling for `QSpinBox` and `QDoubleSpinBox` subcontrols (`::up-button`, `::down-button`, `::up-arrow`, `::down-arrow`) across `style.qss`, `SettingsPageWidget`, `EqualizerWindow`, `SleepTimerDialog`, and `TagEditorDialog`.
- **Atomic Batch Insert Helper**: Added `insert_tracks_batch_tx` helper in `database.rs` to execute slice inserts/updates within a caller-provided SQLite transaction.

#### Changed — Performance & System Responsiveness
- **SQLite Single-Transaction Flushes**: Refactored `flush_new_batch` and `flush_updated_batch` in `library` module to execute batch track flushes within single atomic SQLite transactions, reducing 250 disk fsync operations per batch down to 1 WAL commit and eliminating multi-second disk I/O freezes.
- **O(1) Stale-Track Cleanup**: Optimized `cleanup_missing_tracks` in `db` module to use an in-memory `HashSet` constructed from scanner paths, replacing thousands of sequential `stat()` filesystem syscalls.
- **Single-Pass Folder ID Map**: Refactored `scan_files` in `library` module to pre-build a folder cache map once before file iteration instead of re-querying SQLite per file.
- **Worker Cancellation with Generation Counters**: Introduced atomic `NAV_GENERATION` and `SEARCH_GENERATION` counters in `ui_sync.rs` and `library.rs` to cancel and discard stale background worker results during rapid tab switching or search typing.
- **Throttled CoverLoader & Batch Flush**: Configured a dedicated 2-thread pool (`m_pool`) and a 16 ms batch-flush timer (`m_flushTimer`) in `CoverLoader`, preventing fast scrolling from saturating CPU cores or flooding Qt's event queue.
- **Pruned Worker Handles**: Added `handles.retain(|h| !h.is_finished())` in `app_state.rs` to clean up finished thread handles automatically before spawning new workers.
- **Optimized Playlist Queries**: Rewrote `get_all_playlists` in `db` module to use a single `LEFT JOIN` + `GROUP BY` instead of correlated subqueries.
- **Scaled Default Album Art**: Capped default logo tile allocation to the requested display size (130×130) on first load in `CoverLoader::defaultCover()`.

#### Fixed
- **Cover Art Image Display**: Restored `cached_cover_path(&track.path)` in FFI row payloads (`ui_sync.rs` and `library.rs`), resolving placeholder icon fallbacks in table and grid views.
- **Redundant Tab Refresh**: Removed duplicate `refresh_ui("all")` call on Folders tab navigation.

---

### [1.0.3] — 2026-07-29 — Scalability & Performance Refactor

#### Summary
Architectural performance refactor optimizing PlayTune for 10,000+ track libraries with low CPU/RAM footprint and smooth UI transitions.

#### Added
- **Unified Media Grid (`MediaGridWidget`)**: Shared virtualized grid component for Home, Albums, and Artists tabs with responsive column calculation and lazy image decoding.
- **Process-Wide Cover Loader (`CoverLoader`)**: Off-thread image decoder and LRU cover cache (`QPixmapCache`) shared across all table and grid views.
- **Batch UI Rebuild API (`set_songs_batch`)**: Transactional FFI update API replacing single-track insertions with a single batch transaction.

#### Changed
- **O(1) Table Row Styling**: Optimized table hover and selection styling to repaint only affected rows rather than iterating the entire table.
- **Resize Debouncing**: Deferred grid re-layouts during window drag-resizing to prevent UI stutter.
- **Cover Image Resolution Cap**: Capped embedded album art extraction at 500×500 px to minimize memory and disk cache usage.
- **Memory Footprint Reductions**: Replaced vector clones with move semantics during UI refreshes and search operations.

---

### [1.0.2] — 2026-07-25
#### Added
- **Cross-Platform GitHub Actions CI Workflow**: Configured automated multi-OS matrix builds (`ubuntu-latest`, `macos-latest`, `windows-latest`) in `.github/workflows/ci.yml` running `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo build --release` on push and pull requests.
- **Automated Release Binary Artifact Uploads**: Integrated `actions/upload-artifact@v4` into GitHub CI to package and publish pre-compiled executable binaries (`playtune-linux-x86_64`, `playtune-macos-x86_64`, `playtune-windows-x86_64.exe`) directly to GitHub build run summaries.
- **Linux Desktop Launcher Integration**: Created `assets/playtune.desktop` with standard `StartupWMClass=playtune` and desktop entry metadata for Linux application launchers and taskbars.

#### Fixed
- **Taskbar Application Logo Rendering**: Resolved missing taskbar icon issue on Linux desktop environments and Windows taskbars. Added `app->setDesktopFileName("playtune")`, `app->setOrganizationName("PlayTune")`, and Windows `SetCurrentProcessExplicitAppUserModelID(L"PlayTune.AudioPlayer.1")` in `gui_bridge.cpp`.
- **Clippy & Workspace Code Quality**: Resolved all Clippy linter warnings (`needless_range_loop` in `analysis` and `library`, `empty_line_after_doc_comments` in `engine`, and missing transmute annotations).
- **Unit Test Suite & Dislike Rating Assertion**: Corrected negative rating assertion in `test_ratings` (`db/src/database.rs`) to validate -1 (disliked) rating clamping. All 145 unit tests pass cleanly across the workspace.

---

### [1.0.1] — 2026-07-25
#### Fixed
- **Periodic 10-15s Audio Stumbling**: Resolved periodic audio dropouts/micro-pauses during active playback caused by background device monitor polling false positives.
  - **Root cause**: Background monitor thread (`tc-device-monitor`) queried ALSA audio device configs every 5 seconds. On Linux/PipeWire/PulseAudio, querying an ALSA device while active playback was open caused intermittent sample-rate query flips (44.1kHz vs 48kHz). This sent `EngineCommand::AutoRecoverStream` every ~10-15s, stopping and restarting the active audio output stream mid-song.
  - **Fix**: Guarded `AutoRecoverStream` in `commands.rs` so it ignores background monitor signals during active playback unless CPAL reports an actual hardware stream error (`take_stream_error()`). Refined device monitor sample-rate change detection to ignore ALSA query mode flip-flops when device count and device name are unchanged.
- **Torn Sound / Audio Frame Order Distortion**: Fixed a major audio queue corruption bug in `decode_loop.rs` where unwritten batch samples were being appended to the BACK of `pending_output_frames` instead of the FRONT.
  - **Root cause**: When `output_buffer` became full during batch processing, the unwritten trailing frames from the current batch were being pushed to `pending_output_frames` with `push_back()`. When new frames had already been decoded/resampled, this placed older audio frames *after* future frames in the queue. On the next tick, the audio engine played the future frames first, then jumped back to the delayed frames, causing severe time-displacement, clicking, and torn sound.
  - **Fix**: Changed all unwritten batch fallback pushes in `decode_loop.rs` to insert at the `FRONT` in reverse index order (`for i in (frames_written..batch_fill).rev() { push_front(...) }`). Unwritten frames now remain strictly at the head of the pending queue, preserving 100% accurate chronological audio sample sequence across all buffer stalls.
- **Audio Output Noise / Resampler Overwrite**: Fixed resampler chunk compaction in `resampler.rs` to prevent `rubato::process_into_buffer` from overwriting unconsumed queued samples.

---

### [1.0.0] — 2026-07-24 🎉 OFFICIAL RELEASE
#### Removed
- **Unused Theme Overrides**: Removed non-default theme options (`Cyber Neon`, `Midnight Slate`, `Solar Light`) and associated QSS accent overlay branches, streamlining theme management directly around PlayTune's signature Dark Premium visual identity.

#### Summary of v1.0.0 Release Features
- **Modern Responsive Grid Card Architecture**: Zero-text overlap card scaling, 100% full-coverage thumbnail expander, and unified grid styling across Songs, Albums, and Artists tabs.
- **Enhanced Player Card**: Deep floating 3D drop shadow and 100% fill cover art tile.
- **Optimized Performance & Window Resizing**: Stutter-free 60 FPS drag-resizing with atomic batch paint updates (`setUpdatesEnabled`).
- **Streamlined 2-Column Settings Tab**: Responsive 50/50 split settings grid with zero dark voids.
- **Professional Audio Engine**: Crossfade transitions, EBU R128 loudness normalization, WASAPI/ALSA/ASIO audio backend support, ID3v2 tag editor, M3U/M3U8 playlist import/export, and system tray integration.

---

### [0.9.9] — 2026-07-24
#### Fixed
- **Library Folders Card Void & Stretch Alignment**: Removed `maximumHeight` restriction and added vertical stretch (`stretch = 1`) to `m_foldersListWidget` in `settingspage.cpp`. Tightly packs header and description text at the top of the card and expands the folders list container to fill the card frame smoothly without any dark empty void.

---

### [0.9.8] — 2026-07-24
#### Changed
- **2-Column Settings Layout Restructuring**: Divided the Settings tab into a balanced 2-column split grid:
  - **Left Column**: Appearance & Theme, Add Music To Library (with a neat 2x2 action button grid), and Library Folders.
  - **Right Column**: Playback & Audio Processing and Audio Analysis & ReplayGain.
  Eliminates vertical scrolling and allows all application settings to be viewed comfortably at a glance.

---

### [0.9.7] — 2026-07-24
#### Changed
- **Prominent Search Bar Width**: Increased search bar maximum width to `680px` (minimum `320px`) in `MainWindow`, providing a spacious, premium search input layout.
- **Smooth Jank-Free Window Resizing**: Added `gridSize` unchanged early-exit checks and `setUpdatesEnabled(false/true)` batching across `SongsTableWidget`, `AlbumsViewWidget`, and `ArtistsViewWidget`, eliminating UI stuttering and making window drag-resizing 60 FPS smooth.

---

### [0.9.6] — 2026-07-24
#### Fixed
- **Responsive 3-Dot Action Button Visibility**: Adjusted `setResponsiveWidth` responsive column hiding thresholds in `SongsTableWidget`. Column 6 (the 3-dots action menu) now remains 100% visible across standard, desktop, and tablet window dimensions, only hiding on tiny mobile-constrained widths (< 500px).

---

### [0.9.5] — 2026-07-24
#### Changed
- **Now Playing Player Card Full Space Thumbnail**: Updated `NowPlayingCard` thumbnail renderer to use `Qt::KeepAspectRatioByExpanding`, ensuring artwork fills 100% of the square cover art space cleanly without empty letterboxing or dark bars.
- **Floating Drop Shadow Effect**: Applied `QGraphicsDropShadowEffect` (`blurRadius: 24`, `offset: (0, 6)`, `color: rgba(0,0,0,160)`) to `NowPlayingCard` cover art label, giving the artwork an elevated 3D floating visual appearance.

---

### [0.9.4] — 2026-07-24
#### Changed
- **Songs Grid Card Layout & Breathing Room**: Increased card item height (`coverSize + 78px`) and adjusted cover scale ratio (`cardWidth - 36px`), providing 6px top/bottom padding and a 6px vertical gap between cover artwork and description text. Track title and artist description text are 100% visible, centered, and un-cut across all window dimensions.
- **Unified Grid Proportion Alignment**: Synchronized cover scaling, vertical gaps, and card frame padding identically across **Songs Grid View**, **Albums View**, and **Artists View**.

---

### [0.9.3] — 2026-07-24
#### Changed
- **Zero-Overlap Card Architecture**: Re-engineered `SongGridCard`, `AlbumGridCard`, and `ArtistGridCard` with dynamic `coverSize` scaling and fixed vertical label heights strictly below the cover image, mathematically eliminating text overlapping into thumbnails at any window dimension.
- **100% Full Cover Art Expansion**: Updated `getRoundedPixmap()` to scale artwork using `Qt::KeepAspectRatioByExpanding` so thumbnails fill 100% of the square cover space without letterboxing or dark background bars.
- **Inter-Card Spacing Gaps**: Configured `QListWidget::item` styling (`padding: 6px`) to render crisp, clean `12px` gaps between adjacent card borders across all grid views.
- **Coherent Grid System**: Standardized card layout, rounded corners (radius 14), inner dark frame styling (`#0E121B`), and hover effects identically across **Songs Grid View**, **Albums View**, and **Artists View**.

---

### [0.9.2] — 2026-07-24
#### Changed
- **Action Button Visibility**: Resolved 3-dot action button clipping in `NowPlayingCard` by enabling clean track title text elision, and widened column 6 in `SongsTableWidget` (`48px`) to prevent scrollbar overlap on standard window sizes.
- **Albums & Artists Initial Grid Formatting**: Integrated `showEvent()` and `updateGridResponsive()` handlers in `AlbumsViewWidget` and `ArtistsViewWidget`, ensuring grid cards format in clean responsive columns on app launch before manual window resize.
- **Songs Table Grid View Layout**: Redesigned `SongGridCard` layout proportions to eliminate title/artist text overlapping into thumbnail images, and added dynamic responsive grid math for `m_gridWidget`.

#### Fixed
- **Startup UI Placeholders**: Removed hardcoded demo track metadata ("Midnight Dreams", "Horizon Lines") on initial app launch, replacing them with clean default labels until active playback starts.

---

### [0.9.1] — 2026-07-24
#### Changed
- **Artwork Aspect Ratio & Fit**: Preserved 100% uncut original cover art aspect ratio (`Qt::KeepAspectRatio`) over dark gradient background across Now Playing card, Up Next queue thumbnails, and track table rows.
- **Artists Cards Design**: Updated Artists cards from circular shape (radius 70) to square cards with rounded corners (radius 12) to match Albums cards.
- **Responsive Card Grid**: Implemented dynamic responsive card grid calculation in `AlbumsViewWidget` and `ArtistsViewWidget`, smoothly scaling card widths and grid columns to fit standard, tablet, and fullscreen window dimensions.
- **Songs Table Header & Row Alignment**: Aligned column header text with table cells, added favorite column header icon, and vertically centered cell action buttons (`♥`/`♡` favorite toggle & 3-dots action menu).
- **Frameless Dark Floating Panels**: Converted `SleepTimerDialog` and `PlaylistCreateDialog` into frameless dark floating overlay cards matching application design system.
- **Dark Theme QMenu Context Menus**: Applied dark glassmorphic styling to all `QMenu` context menus across the application.
- **Playback Control Buttons**: Increased button sizes for Repeat, Shuffle, Equalizer, and Sleep Timer controls on `NowPlayingCard`.
- **Centered Search Bar**: Horizontally centered search bar above `NowPlayingCard` relative to the central panel layout.
- **Albums & Artists First-Track Thumbnail**: Automatically fetched and rendered first-track cover artwork on Albums and Artists cards.

#### Fixed
- **Queue Track Rearrange & Metadata Preservation**: Synchronized queue reordering between Qt GUI (`DragDropQueueTableWidget`) and Rust backend state (`CURRENT_TRACK_LIST` & `CURRENT_INDEX`), preserving track metadata, thumbnails, and playability across repeated drag operations.
- **Queue Track Numbers**: Fixed track number visibility and dynamic re-indexing beside Up Next queue items.

---

### [0.9.0] — 2026-07-24
#### Added
- **Standardized Versioning & CHANGELOG**: Established project-wide 3-way (`x.y.z`) Semantic Versioning specification and master `CHANGELOG.md`.
- **Workspace Version Bump**: Synchronized all 7 core workspace modules (`playtune`, `engine`, `gui`, `library`, `db`, `config`, `platform`, `analysis`) to version `0.9.0`.
- **Frameless EQ Workstation Geometry Persistence**: `EqualizerWindow` saves physical screen position `(X, Y)` and dimensions `(Width, Height)` to `QSettings`, restoring geometry seamlessly across app restarts.
- **Global App Tooltip Toggle**: Integrated `QEvent::ToolTip` application event filter toggleable from Settings, providing clean parameter readouts (`Preamp dB`, `Stereo Width %`, `EQ Frequencies`) app-wide.
- **65-Bin Real-Time FFT Visualizer**: Real-time logarithmic frequency spectrum tap (`realfft`) operating on the 30ms ticker thread with fallback idle breathing wave animation.

#### Changed
- Refactored release profile in `Cargo.toml` (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"`) for optimal binary footprint and low-latency DSP execution.
- Improved playback progress accuracy and auto-advance threshold calculation (capped at 50% of track length for short audio jingles).

#### Fixed
- Fixed integer truncation issues in sample rate converter (`resampler.rs: #28`).
- Fixed track ID borrow lifetime bug in MPRIS D-Bus bridge (`dbus.rs`).
- Fixed main loop thread contention on ALSA device enumeration using silent error handler hook.

---

### [0.8.5] — 2026-07-22
#### Added
- ReplayGain active peak indicator badge on `NowPlayingCard` when currently playing track contains valid embedded ReplayGain tags.

#### Fixed
- Fixed ReplayGain LUFS calculation on 24-bit and 32-bit high-resolution audio files (`24-bit/96kHz` & `24-bit/192kHz`).

---

### [0.8.4] — 2026-07-15
#### Performance & Fixed
- Optimized Lookahead Peak Limiter delay ring buffer allocations statically per sample rate (`44.1k - 192k`).
- Fixed soft-knee compression curve calculation in multiband compressor at ratio settings below `2:1`.

---

### [0.8.3] — 2026-07-18
#### Added
- **Loudness Scan Cancellation**: Added atomic cancellation guard support (`playtune_cancel_loudness_scan`) allowing users to safely abort long-running background scan operations in `LoudnessScannerDialog`.
- Batch insertion of scanned LUFS metadata directly into SQLite WAL database within single atomic SQL transactions.

#### Fixed
- Fixed ReplayGain tag parser handling for non-standard capitalized `REPLAYGAIN_TRACK_GAIN` and `replaygain_track_gain` tags in FLAC Vorbis Comments.

---

### [0.8.2] — 2026-07-10
#### Fixed
- Fixed true peak clipping guard on positive ReplayGain boosts surpassing `-0.2 dBFS`.
- Added support for Album Gain vs. Track Gain mode toggle in `LoudnessScannerDialog`.
- Optimized multi-threaded loudness scanner progress bar updates to prevent Qt UI thread event queue congestion.

---

### [0.8.1] — 2026-06-28
#### Added
- **TPDF Word-Length Dithering**: Added Triangular Probability Density Function (`TPDF`) dithering with psychoacoustic noise shaping for 16-bit DAC output delivery (`dither.rs`).

#### Fixed
- Fixed multi-band compressor frequency crossover phase alignment using phase-matched Linkwitz-Riley 4th order filters.
- Prevented potential buffer underrun when rapidly altering compressor attack/release sliders.

---

### [0.8.0] — 2026-06-15
#### Added
- **Multi-Threaded Loudness Scanner & Tag Writer**: `LoudnessScannerDialog` and background worker threads (`loudness_scanner.rs`) for rapid batch LUFS, True Peak, and Loudness Range (`LRA`) calculation.
- **ReplayGain 2.0 Normalization**: Real-time gain scaling supporting both **Track Gain** (shuffled tracks) and **Album Gain** (concept albums) targeting `-18 LUFS` / `-23 LUFS`.
- **Multi-Band Dynamics Compressor**: 3-band Linkwitz-Riley crossover splitter with independent ratio, threshold, attack, release, and make-up gain controls (`multiband_compressor.rs`).
- **Lookahead Peak Limiter**: Brickwall inter-sample peak protection preventing DAC clipping (`limiter.rs`).

---

### [0.7.5] — 2026-06-12
#### Added
- Support for loading stereo 32-bit floating point `.wav` Impulse Response files into Partitioned Convolution Engine.
- Wet/Dry gain blend slider (`0% - 100%`) for partitioned convolution room acoustic simulation.

---

### [0.7.4] — 2026-06-08
#### Fixed
- Fixed Jan Meier crossfeed high-frequency shelf filter response curve above `12 kHz`.
- Resolved phase cancellation artifact when combining 3D stereo width expansion with binaural crossfeed.

---

### [0.7.3] — 2026-06-05
#### Performance & Fixed
- Added IR impulse response memory cache in `convolution.rs` to avoid reloading and re-partitioning identical `.wav` files when toggling convolution profiles.
- Fixed channel mismatch handling when loading mono impulse response files into a stereo processing pipeline.

---

### [0.7.2] — 2026-05-25
#### Performance & Fixed
- Added head partition optimization (`64-sample` block size) to Partitioned Convolution Engine for zero-latency impulse response processing.
- Fixed impulse response file sample rate auto-resampling when loading external `.wav` IR files recorded at rates non-matching native DAC output.

---

### [0.7.1] — 2026-05-12
#### Added
- **Siegfried Linkwitz Crossfeed Model**: Added inter-aural time delay (`300 µs`) acoustic shadowing crossfeed profile alongside Chu-Moy and Jan Meier profiles (`crossfeed.rs`).

#### Fixed
- Fixed 3D stereo width Mid/Side processing balance clamping at extreme `200%` width expansion settings.

---

### [0.7.0] — 2026-05-02
#### Added
- **Partitioned Convolution Engine**: Zero-latency impulse response (`.wav`) processor for studio room acoustic modeling and headphone EQ calibration (`convolution.rs`).
- **Binaural Spatial Crossfeed**: Integrated Chu-Moy, Jan Meier, and Siegfried Linkwitz binaural models for reducing headphone acoustic fatigue (`crossfeed.rs`).
- **Mid/Side 3D Stereo Width Expansion**: Spatial field processing allowing stereo widening from `0%` (mono) to `200%` (wide).

---

### [0.6.5] — 2026-05-01
#### Added
- Support for ID3v2.3 `TRCK` and `TPOS` multi-disc parsing (e.g., `1/2` track and disc numbering).

#### Fixed
- Improved `TagEditorDialog` validation for invalid year inputs (`0 - 9999`).

---

### [0.6.4] — 2026-04-22
#### Added
- Support for `.lrc` timestamp lines containing multiple timestamp prefixes (`[01:12.50][02:40.10] Chorus line`).

#### Fixed
- Fixed scrolling lyric smooth animation lag in `KaraokeDialog` on high refresh rate monitors (`144 Hz+`).

---

### [0.6.3] — 2026-04-28
#### Added
- Support for ID3v2.4 UTF-8 frame encoding in `TagEditorDialog` when writing custom comment or artist metadata back to disk.

#### Fixed
- Fixed GUI table refresh artifact where modified tags did not immediately reflect in sorted `SongsTable` columns.

---

### [0.6.2] — 2026-04-18
#### Added
- **Manual Seek-by-Lyric**: Clicking any timestamped line inside `KaraokeDialog` jumps track position immediately to that exact vocal entry point.

#### Fixed
- Fixed `LrcParser` handling of non-standard lyric timestamps (`[mm:ss.xxx]` and sub-second 3-digit millisecond fractions).

---

### [0.6.1] — 2026-04-02
#### Added
- Support for embedded Vorbis Comment `LYRICS` and ID3 `SYLT` (Synchronized Lyrics) tag extraction.

#### Fixed
- Fixed batch tag saving in `TagEditorDialog` when updating album cover art across multiple tracks simultaneously.

---

### [0.6.0] — 2026-03-20
#### Added
- **Dedicated Folder View (`FoldersView`)**: Directory tree filesystem browser allowing navigation and track playback without relying on metadata tags (`foldersview.cpp`).
- **Synchronized Lyrics Viewer (`KaraokeDialog`)**: Full-screen scrolling lyrics display with dynamic gradient highlighting and click-to-seek functionality.
- **`LrcParser` Engine**: Automated parsing of timestamped `.lrc` external files and embedded `USLT`/`SYLT`/Vorbis lyrics.
- **Metadata Tag Editor (`TagEditorDialog`)**: Direct ID3v1/ID3v2, Vorbis Comment, and MP4 atom editor with disk write-back and instant DB sync.

---

### [0.5.5] — 2026-03-18
#### Added
- Windows SMTC (SystemMediaTransportControls) thumbnail artwork synchronization.

#### Fixed
- Resolved MPRIS metadata update frequency throttling during high-speed track skipping.

---

### [0.5.4] — 2026-03-08
#### Fixed
- Added global atomic `SHUTDOWN` flag checks inside long-running database loop operations.
- Fixed thread-safety guard on `PLATFORM` static mutex during OS media key callback handling.

---

### [0.5.3] — 2026-03-12
#### Added
- Automatic cover art cache garbage collection removing orphaned `.png` thumbnails from disk.

#### Fixed
- Fixed MPRIS `Seeked` signal emissions on Linux D-Bus when user manually drags progress slider in `MainWindow`.

---

### [0.5.2] — 2026-03-05
#### Fixed
- Added SIGINT graceful shutdown handler (`ctrlc`) and worker thread join-watcher pool (`WORKER_HANDLES`) preventing lingering background processes.
- Fixed MPRIS track position microsecond overflow calculation for audio tracks longer than 1 hour.

---

### [0.5.1] — 2026-02-22
#### Added
- Desktop media key command support for Shuffle Toggle, Repeat Mode, Volume Up/Down, and relative Seek actions across Linux MPRIS & Windows SMTC.

#### Fixed
- Fixed MPRIS artwork `file://` URI formatting for cached album art thumbnails.

---

### [0.5.0] — 2026-02-10
#### Changed
- **Architecture Refactoring**: Refactored monolithic `main.rs` into clean, decoupled modules (`app_state.rs`, `bridge.rs`, `ui_sync.rs`, `handlers/playback.rs`, `handlers/library.rs`, `handlers/settings.rs`).
- **Native OS Media Keys (`souvlaki`)**: Deep integration with Linux MPRIS D-Bus, Windows SMTC, and macOS MPRemoteCommandCenter for desktop media key controls and system notification metadata.

---

### [0.4.5] — 2026-02-18
#### Performance & Fixed
- Added SQLite index optimization on `play_count` and `last_played_at` columns for `Most Played` and `Recently Played` smart views.
- Added automatic database WAL truncation (`PRAGMA wal_checkpoint(TRUNCATE)`) on application exit.

---

### [0.4.4] — 2026-02-05
#### Added
- Single-pass embedded artwork resizing and PNG caching during `walkdir` background indexing.

#### Fixed
- Fixed handling of special characters (quotes, apostrophes, ampersands) in SQLite search queries.

---

### [0.4.3] — 2026-02-01
#### Added
- Automated cleanup of legacy mock database tracks (`delete_mock_tracks`) on background startup thread.

#### Fixed
- Improved SQLite index coverage on `(artist, album)` composite keys for sub-millisecond filtering.

---

### [0.4.2] — 2026-01-25
#### Performance & Fixed
- Implemented `150ms` debounced global search across Title, Artist, Album, and Genre columns in `SongsTable`.
- Fixed SQLite WAL checkpointing overhead during multi-folder recursive indexing (`PRAGMA synchronous = NORMAL;`).

---

### [0.4.1] — 2026-01-08
#### Added
- Recursive file and directory drag-and-drop ingestion onto main playlist tables and side queue drawer (`QueueWidget`).

#### Fixed
- Fixed embedded cover art extraction for WebP and PNG formats using the `image` crate.

---

### [0.4.0] — 2025-12-18
#### Added
- **SQLite Write-Ahead Logging (`WAL`) Mode**: High-concurrency database engine enabling background library scanning while searching and streaming with zero lock contention (`database.rs`).
- **Single-Pass `walkdir` Indexer**: Single disk pass metadata & album cover art extraction (`image` crate) with atomic batch SQL insertions (`100+ items/batch`).
- **Smart Playlists**: Built-in dynamic database views for *Favorites*, *Recently Played*, and *Most Played*.

---

### [0.3.5] — 2025-12-15
#### Added
- Support for direct ALSA hardware device selection (`hw:0,0`, `hw:1,0`) in Settings.

#### Fixed
- Fixed CoreAudio Hog Mode release guard on macOS when closing the application unexpectedly.

---

### [0.3.4] — 2025-12-08
#### Added
- Automatic CPAL audio host driver detection across ALSA, PulseAudio, PipeWire, WASAPI, and ASIO.

#### Fixed
- Fixed output stream channel map fallback when playing 5.1/7.1 surround sound FLAC files on stereo DACs.

---

### [0.3.3] — 2025-12-02
#### Fixed
- Added direct hardware sample rate negotiation on ALSA output stream initialization.
- Fixed CPAL output stream re-initialization deadlock when switching output devices while playing.

---

### [0.3.2] — 2025-11-20
#### Fixed
- Added automatic fallback from Exclusive Mode to Shared Mode when DAC hardware does not support requested exclusive sample rates.
- Reduced device monitor polling overhead and prevented duplicate stream rebuild invocations.

---

### [0.3.1] — 2025-11-05
#### Fixed
- Fixed WASAPI Exclusive fallback behavior on unsupported sample rates.
- Resolved CPAL output buffer lock contention during device hot-swaps.

---

### [0.3.0] — 2025-10-12
#### Added
- **Bit-Perfect Output Drivers**: Exclusive hardware backends for WASAPI Exclusive (Windows), ASIO (Windows Studio), Direct ALSA (Linux), and CoreAudio Hog Mode (macOS).
- **Device Hot-Swapping (`tc-device-monitor`)**: 5-second polling background thread with sub-5ms stream renegotiation during USB/Bluetooth disconnects.

---

### [0.2.5] — 2025-10-08
#### Added
- Tab 3 Parametric EQ filter shapes: `Low Pass`, `High Pass`, `Bandpass`, and `Notch` filters.
- Real-time biquad magnitude curve plotting in `EqualizerWindow`.

---

### [0.2.4] — 2025-10-01
#### Added
- Pre-amplification trim gain control (`-20 dB` to `+20 dB`) and Master Balance slider in EQ Tab 2.

#### Fixed
- Fixed biquad filter peak gain normalization to prevent digital clipping when boosting narrow Q parametric bands.

---

### [0.2.3] — 2025-09-25
#### Added
- Independent Tab-level reset functionality: resetting parameters in one EQ tab preserves settings in other tabs.

#### Performance
- Optimized Catmull-Rom spline evaluator to pre-calculate spline nodes during slider movement instead of recalculating per-frame.

---

### [0.2.2] — 2025-09-18
#### Added
- 7 Curated Genre Presets (*Flat, Pop, Rock, Jazz, Classical, Electronic, Hip Hop*) for Graphic EQ.

#### Fixed
- Upgraded biquad filter internal state variables to 64-bit double precision (`f64`) to prevent low-frequency quantization noise.

---

### [0.2.1] — 2025-09-05
#### Added
- Catmull-Rom spline interpolation for smooth graphic EQ frequency response curves.
- 4 Selectable quality tiers in `rubato` sample rate converter (*Fast*, *Balanced*, *High Quality 4x*, *Ultra HD*).

---

### [0.2.0] — 2025-08-30
#### Added
- **`rubato` High-Fidelity Resampling**: Polyphase & Catmull-Rom sample rate conversion with 4 selectable quality tiers (*Fast*, *Balanced*, *High Quality 4x*, *Ultra HD*).
- **10-Band ISO Graphic Equalizer**: Catmull-Rom spline interpolated frequency response curve editor with 7 genre presets.
- **10-Band Parametric Equalizer**: Precision biquad editor with configurable Q factors (`0.1 - 24.0`), center frequencies, and 7 filter shapes.

---

### [0.1.6] — 2025-08-28
#### Added
- AAC (`.m4a`) gapless playback decoding via Symphonia AAC edit-list atom reader.

#### Performance
- Improved CPAL buffer delivery latency on low-end dual-core hardware.

---

### [0.1.5] — 2025-08-18
#### Added
- Volume logarithmic curve mapping to provide natural human perception scalar volume control.

#### Fixed
- Fixed initial window position center placement on multi-monitor desktop setups.

---

### [0.1.4] — 2025-08-22
#### Fixed
- Added automatic output stream recovery when audio device buffer underflow (`CPAL BufferUnderrun`) occurs.
- Added FLAC native sample-exact header duration parser.

---

### [0.1.3] — 2025-08-15
#### Added
- Lock-free crossbeam ring-buffer channel between audio decoder thread and CPAL callback renderer.
- Custom dark tooltips for volume and playback progress sliders.

---

### [0.1.2] — 2025-08-10
#### Fixed
- Fixed sample-accurate gapless playback transitions between tracks using LAME delay/padding calculation for MP3 and exact sample counts for FLAC.
- Added audio buffer flush-to-zero denormal flag configuration on ticker worker thread.

---

### [0.1.1] — 2025-07-20
#### Added
- Lock-free `ArcSwap<PlaybackInfo>` atomic state bridge between Rust audio thread and C++ Qt GUI.
- Initial dark glassmorphism styling stylesheet (`style.qss`).

---

### [0.1.0] — 2025-07-01
#### Added
- Initial pre-alpha release of **PlayTune (TuneCraft)**.
- Pure-Rust `symphonia` audio decoder (MP3, FLAC, WAV, AAC, OGG Vorbis, Opus, ALAC).
- Zero-allocation hot path audio engine on top of `cpal`.
- Native C++ Qt6 Dark Glassmorphic user interface.

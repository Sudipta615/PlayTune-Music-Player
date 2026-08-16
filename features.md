# 🎛️ PlayTune / TuneCraft: Comprehensive Feature Specification & Architecture Guide

<div align="center">

![PlayTune Logo](assets/logo.png)

**An Uncompromising Audiophile Music Player Powered by a Native C++ Qt6 GUI & a Zero-Allocation Lock-Free Rust Audio Engine**

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-FF4B4B.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Qt6](https://img.shields.io/badge/Qt6-Native_GUI-41CD52.svg?style=for-the-badge&logo=qt&logoColor=white)](https://www.qt.io/)
[![DSP Engine](https://img.shields.io/badge/DSP-Zero_Allocation_Hot_Path-8A2BE2.svg?style=for-the-badge)](#)
[![Drivers](https://img.shields.io/badge/Output-WASAPI_Exclusive_%7C_ASIO_%7C_ALSA-00F2FE.svg?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-Apache--2.0-0052FF.svg?style=for-the-badge)](LICENSE)

</div>

---

## 📖 Executive Summary & Architectural Vision

Modern desktop music players often force users to make a compromise between **fidelity**, **performance**, and **user experience**. Electron-based web players consume hundreds of megabytes of idle memory, suffer from garbage collection stutters, and route audio through generic system software mixers. Conversely, classic audiophile players provide high audio quality but often rely on outdated UI frameworks, complex modular plugin dependencies, or high-latency processing chains.

**PlayTune (internally codenamed TuneCraft)** eliminates this compromise through a strict, hybrid separation of concerns:
- **The Frontend (`modules/gui`)** is built in native C++ using **Qt6**, delivering sub-millisecond visual feedback, sleek glassmorphism styling, real-time 65-bin FFT visualization, and custom tooltips without running any heavy audio or database business logic.
- **The Backend (`modules/engine`, `modules/db`, `modules/library`)** is written entirely in **memory-safe Rust**, executing over an ultra-fast C/FFI bridge. It enforces a strict **zero-allocation hot path** where all digital signal processing (DSP), sample rate conversion, decoding, and CPAL buffer delivery occur inside pre-allocated lock-free ring and scratch buffers (`ArcSwap<PlaybackInfo>`, `OnceLock`, `crossbeam`).

```
+-----------------------------------------------------------------------------------+
|                              C++ Qt6 NATIVE FRONTEND                              |
|   MainWindow | EqualizerWindow | KaraokeDialog | TagEditorDialog | NowPlayingCard |
|   FoldersView | SongsTable     | Sidebar       | QueueWidget     | CustomWidgets  |
+-----------------------------------------------------------------------------------+
       ^                 |                                   ^
       | Lock-Free State | C/FFI Atomic Commands             | 65-Bin FFT
       | (ArcSwap)       | (Crossbeam Channels)              | Ring Buffers
       v                 v                                   |
+-----------------------------------------------------------------------------------+
|                            RUST AUDIO & ENGINE CORE                               |
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  |                           DSP PIPELINE ENGINE                               |  |
|  |  Symphonia Decoding -> Resampler (Rubato) -> Parametric/Graphic EQ ->       |  |
|  |  Convolution Engine -> Binaural Crossfeed -> Lookahead Limiter -> Dither    |  |
|  +-----------------------------------------------------------------------------+  |
|                                                                                   |
|  +------------------------+  +-------------------------+  +--------------------+  |
|  |  CPAL OUTPUT DRIVERS   |  |   SQLITE WAL DATABASE   |  |   DEVICE MONITOR   |  |
|  |  WASAPI Excl / ASIO /  |  |   Concurrent WAL Mode / |  |   5s Polling &     |  |
|  |  Direct ALSA / Hog     |  |   Walkdir Single-Pass   |  |   Auto Hot-Swap    |  |
|  +------------------------+  +-------------------------+  +--------------------+  |
+-----------------------------------------------------------------------------------+
```

---

## 🌟 Comprehensive Feature Matrix at a Glance

| Category | Feature Name | Key Specifications & Capabilities |
| :--- | :--- | :--- |
| **Audio Output** | **Bit-Perfect Output Drivers** | Direct hardware DAC access via **WASAPI Exclusive** (Win), **ASIO** (Win Hardware), **Direct ALSA** (Linux), and **CoreAudio Hog Mode** (macOS). Bypasses OS software mixers. |
| **Audio Output** | **Device Hot-Swapping & Recovery** | `tc-device-monitor` polls audio endpoints every `5s`. Automatically renegotiates sample rates and recovers streams within `5ms` during USB/Bluetooth disconnects. |
| **Audio Engine** | **Zero-Allocation Hot Path** | 100% pre-allocated scratch & ring buffers. Zero heap allocations (`Vec::new`, `Box::new`, `clone`) inside the audio callback or DSP chain. |
| **Audio Engine** | **Symphonia Multi-Format Decoding** | Pure memory-safe Rust decoding for MP3, FLAC, WAV, AAC, OGG Vorbis, and Opus without vulnerable C audio codec wrappers (`ffmpeg`). |
| **DSP Suite** | **3-Tab Segmented Equalizer Workstation** | **Tab 1:** 10-Band Graphic EQ (ISO bands, Catmull-Rom splines, 7 genre presets).<br/>**Tab 2:** Tone/Spatial (Bass/Treble shelves, Stereo Width `0-200%`, Balance, Preamp).<br/>**Tab 3:** Parametric EQ (`0.1-24.0 Q`, 7 filter shapes) + Resampler Quality selection. |
| **DSP Suite** | **Floating Resizable/Movable EQ Window** | Frameless Qt window (`FramelessWindowHint`). Completely draggable across monitors, resizable (`650x480` to full screen), and persists exact geometry coordinates to `QSettings`. |
| **DSP Suite** | **High-Fidelity Resampling (`rubato`)** | Polyphase / Catmull-Rom sample rate conversion with 4 selectable tiers: *Fast*, *Balanced*, *High Quality 4x*, and *Ultra HD*. |
| **DSP Suite** | **Real-Time Convolution Engine** | Zero-latency / partitioned convolution processor loading `.wav` Impulse Response (IR) files for studio room acoustics, reverb, and headphone calibration. |
| **DSP Suite** | **Binaural Spatial Crossfeed** | High/low frequency crossfeed (`Chu-Moy`, `Jan Meier`, `Linkwitz` models) reducing acoustic fatigue when listening on headphones. |
| **DSP Suite** | **Multi-Band Dynamics Compressor** | Active spectrum crossover filters splitting audio into frequency bands with independent threshold, ratio, attack, and release parameters. |
| **DSP Suite** | **Lookahead Limiter & TPDF Dithering** | Brickwall lookahead inter-sample peak limiting + 16-bit/24-bit Triangular Probability Density Function (`TPDF`) dithering with noise shaping. |
| **General / UX** | **File & Folder Adding System** | Interactive single-file (`Ctrl+O`) and recursive multi-directory (`Ctrl+Shift+O`) import alongside drag-and-drop ingestion onto main tables and queue drawers. |
| **General / UX** | **Dedicated Folder View (`FoldersView`)** | Direct hierarchical filesystem tree browser (`foldersview.cpp`) allowing instant navigation and playback of directory folders without relying on ID3 tags. |
| **General / UX** | **Smart Playlists & Queue Management** | Dynamic views for *Favorites*, *Recently Played*, and *Most Played*. Interactive sidebar queue (`QueueWidget`) with drag-and-drop reordering and cover art thumbnails. |
| **General / UX** | **Debounced Global Search (`Ctrl+F`)** | High-speed debounced (`150ms`) search querying Title, Artist, Album, and Genre across thousands of database rows instantaneously. |
| **Loudness** | **EBU R128 & ReplayGain 2.0 Normalization** | Real-time gain scaling matching `-18 LUFS` / `-23 LUFS` targets. Supports both **Track Gain** (for shuffled playlists) and **Album Gain** (for concept albums). |
| **Loudness** | **Built-in Loudness Scanner & Writer** | Multi-threaded background loudness scanner analyzing LUFS, True Peak, and Loudness Range (`LRA`), embedding standard ReplayGain tags directly into audio files. |
| **Metadata** | **Built-in Metadata Tag Editor** | Interactive UI (`TagEditorDialog`) for inspecting, modifying, and saving ID3v1/ID3v2, Vorbis, and MP4 tags (Title, Artist, Album, Disc/Track #, Cover Art). |
| **Lyrics** | **Synchronized Lyrics Viewer (`KaraokeDialog`)** | Real-time synchronized lyrics display powered by `LrcParser` reading external `.lrc` timestamped files and embedded ID3/Vorbis lyrics (`USLT`/`SYLT`). |
| **Library & DB** | **Concurrent Lock-Free SQLite WAL Library** | Write-Ahead Logging (`WAL`) mode enabling simultaneous background folder indexing, searching, queuing, and streaming without lock contention. |
| **Library & DB** | **Single-Pass Recursive Extraction** | `walkdir` scan pulling ID3/Vorbis tags + embedded album art (`image` crate) in one disk pass. Sub-millisecond transactional batch inserts (`100+ items/batch`). |
| **UI / Visuals** | **Live 65-Bin Real-Time FFT Spectrum** | Zero-UI-stall frequency spectrum calculated via `realfft` on the backend ticker thread and dispatched smoothly to the Qt6 `NowPlayingCard` canvas. |
| **UI / Visuals** | **Dark Glassmorphism & Custom Tooltips** | Sleek dark-mode glass styling with custom parameter readout tooltips (`QEvent::ToolTip` filter toggleable globally from Settings). |
| **OS Bridge** | **Deep Desktop Media & Key Integration** | Native integration with **Linux MPRIS D-Bus**, **Windows SMTC**, and **macOS MPRemoteCommandCenter** for media keys, taskbar overlays, and lock screen controls. |

---

## 🔍 General Features & Library Ergonomics

PlayTune is crafted not only for acoustic perfection but also for seamless day-to-day usability. It handles massive libraries effortlessly while providing intuitive, lightning-fast navigation workflows.

### 1. File / Folder Adding & Import System (`walkdir` Single-Pass Engine)
To build a library without locking the user interface, PlayTune provides versatile file ingestion paths:
- **Direct Dialog Imports:** Users can add individual tracks via **Add File (`Ctrl+O`)** or ingest entire multi-level hierarchies via **Add Folder (`Ctrl+Shift+O`)**.
- **System Drag-and-Drop:** Dragging files or directories directly from the OS file manager (`Dolphin`, `Explorer`, `Finder`) onto the PlayTune window or playlist table immediately enqueues them for background scanning.
- **Asynchronous Non-Blocking Indexing:** When a folder is added, the `library/src/lib.rs` worker pool takes over in a background thread. It uses `walkdir` to traverse directories, extracts audio tags and cover art in a single disk read pass (`metadata.rs`, `cover_art.rs`), and flushes new records to the SQLite database in atomic batch transactions of `100+ tracks`. The UI remains at a locked `60 FPS` even when importing tens of thousands of files.

### 2. Dedicated Folder View System (`FoldersView`)
While metadata tags (`ID3v2 / FLAC tags`) are standard, many audiophiles organize live bootlegs, multi-disc box sets, and rare vinyl rips purely by directory structure. PlayTune features a dedicated **Folder View (`foldersview.cpp`)**:
- Renders a clean, hierarchical tree view of all imported storage directories (`/mnt/Music/FLAC_Collection/...`).
- Clicking any directory displays the exact files within that specific folder path without cluttering the view with unrelated album matches.
- Users can right-click any directory in `FoldersView` to instantly **Play Folder** or **Add Folder to Queue**, preserving the exact file track ordering (`01 - Track.flac`, `02 - Track.flac`).

### 3. Smart Playlists & Interactive Queue Drawer (`SongsTable`, `QueueWidget`)
PlayTune categorizes and manages tracks through dynamic, database-driven views and an ergonomic side queue:
- **Smart Built-in Views:**
  - `Favorites`: Instant one-click star filter retrieving all starred tracks (`SELECT * FROM songs WHERE is_favorite = 1`).
  - `Recently Played`: Chronological history of listening activity sorted by exact timestamps (`last_played_at DESC`).
  - `Most Played`: Automatically ranks tracks based on cumulative listen counts (`play_count DESC`), giving users instant access to their daily heavy rotation.
  - `All Songs`: Full high-speed tabular view (`songstable.cpp`) supporting instant column sorting by Title, Artist, Album, Duration, Bitrate (`kbps`), and Sample Rate (`kHz`).
- **Interactive Queue Drawer (`QueueWidget`):**
  - A slide-out side panel displaying all upcoming queued tracks complete with thumbnail album artwork and exact durations.
  - Supports smooth drag-and-drop track reordering, immediate swipe/click removal, and context menu options to **Play Next** (inserting a track immediately after the currently playing item) or clear the remaining queue.

### 4. Debounced Global Search (`Ctrl+F`)
- Pressing `Ctrl+F` instantly focuses the global search bar at the top of the interface.
- To prevent UI stutter while typing queries against large databases, the search input applies a **150ms debounce filter**.
- As soon as typing pauses, the query executes concurrently across `title`, `artist`, `album`, and `genre` columns using SQLite indexed lookups, rendering matching tracks instantaneously.

---

## 🔬 In-Depth Engineering Deep Dive: The Audiophile DSP Engine (`modules/engine/src/dsp`)

The digital signal processing (DSP) pipeline is the heart of PlayTune. Unlike standard players that execute simple volume scalar multiplications and basic biquads directly on the UI thread or within shared system buffers, PlayTune executes a comprehensive, highly modular, lock-free DSP processing graph (`pipeline.rs`). Every stage in this graph is engineered to ensure total mathematical accuracy while strictly preserving real-time performance invariants.

```
                  +------------------------------------------------+
                  |           SYMPHONIA AUDIO DECODER              |
                  |  (Raw 32-bit Floating Point Audio Sample Plan) |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |         SAMPLE RATE CONVERSION (RUBATO)        |
                  |  (Asynchronous/Synchronous Catmull-Rom Poly)   |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |             GAIN & SPIKE PROTECTION            |
                  |  (Preamp Gain -> Acoustic volume.snap() Guard) |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |       LOUDNESS & REPLAYGAIN 2.0 SCALING        |
                  |  (EBU R128 / ReplayGain Track & Album Scaling) |
                  +------------------------------------------------+
                                          |
                                          v
      +-----------------------------------+-----------------------------------+
      |                                                                       |
      v [If Parametric EQ Active]                                             v [If Graphic EQ Active]
+------------------------------------+                                  +------------------------------------+
|       PARAMETRIC EQUALIZER         |                                  |         GRAPHIC EQUALIZER          |
|  10 Precision Cascaded Biquads     |                                  |  10 ISO Bands + Spline Interp      |
|  (Peaking, Shelves, Pass/Notch)    |                                  |  (7 Curated Genre Presets)         |
+------------------------------------+                                  +------------------------------------+
      |                                                                       |
      +-----------------------------------+-----------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |            MULTI-BAND COMPRESSOR               |
                  |  (Frequency Split Crossover -> Dynamic Ratio)  |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |         PARTITIONED CONVOLUTION ENGINE         |
                  |  (Zero-Latency IR Room & Reverb Simulation)    |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |         BINAURAL SPATIAL CROSSFEED             |
                  |  (Chu-Moy / Jan Meier / Linkwitz Profiles)     |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |       STEREO FIELD & WIDTH MANIPULATION        |
                  |  (Mid/Side Processing, Width 0% to 200%)       |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |     CROSSFADING & SEAMLESS TRACK BLENDING      |
                  |  (Linear / Constant Power / Log / S-Curve)     |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |       LOOKAHEAD PEAK LIMITER (BRICKWALL)       |
                  |  (Inter-Sample Peak Guard against Clipping)    |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |        WORD-LENGTH REDUCTION DITHERING         |
                  |  (16/24-Bit TPDF Dithering & Noise Shaping)    |
                  +------------------------------------------------+
                                          |
                                          v
                  +------------------------------------------------+
                  |              CPAL OUTPUT DRIVERS               |
                  |  (WASAPI Exclusive / ASIO / ALSA / CoreAudio)  |
                  +------------------------------------------------+
```

### 1. The Zero-Allocation Hot Path Invariant
A foundational rule across all `engine/src/dsp` modules (`pipeline.rs`, `buffer.rs`) is: **Zero heap allocation (`Vec::new()`, `Box::new()`, `clone()`) during audio processing.**
- When the CPAL hardware output stream requests a block of audio (`encode_and_process`), the engine operates exclusively within pre-allocated ring buffers and static scratch vectors that are initialized during stream startup (`Pipeline::new()`).
- If an audio file requires sample rate conversion, filtering, or crossfading, the intermediate sample frames are transformed in-place inside these scratch buffers. This guarantees deterministic execution times (`< 0.8ms` per frame) and eliminates Rust allocators (`jemalloc`/`glibc`) or OS kernel memory management interruptions.

### 2. The 3-Tab Segmented Equalizer & DSP Workstation (`equalizer.rs`, `EqualizerWindow`)
To provide both quick adjustments for causal listening and surgical precision for mastering engineers, PlayTune replaces traditional cluttered sliders with a context-aware **3-Tab Workstation Window**:

#### 🎚️ Tab 1: 10-Band Graphic Equalizer
- Features 10 standard ISO frequency bands: `31 Hz`, `63 Hz`, `125 Hz`, `250 Hz`, `500 Hz`, `1 kHz`, `2 kHz`, `4 kHz`, `8 kHz`, and `16 kHz`.
- Unlike basic digital linear interpolators that introduce phase distortion and sharp corner artifacts across sliders, PlayTune applies **Catmull-Rom spline interpolation** (`equalizer.rs`) to construct smooth, natural frequency response curves across all cascaded biquad stages.
- Includes **7 Curated Genre Presets**: *Flat, Pop, Rock, Jazz, Classical, Electronic, and Hip Hop*, alongside custom user memory persistence stored via `modules/config`.

#### 🎛️ Tab 2: Tone & Spatial Controls
- **Bass & Treble Shelf Filters**: High-precision low-shelf and high-shelf biquads allowing broad-stroke warmth or shimmer adjustments without touching detailed EQ curves.
- **3D Stereo Width Expansion**: Controls the spatial spread of the stereo image from `0%` (pure mono summing) to `100%` (unaltered stereo) up to `200%` (Mid/Side processing where side channel energy is boosted for an ultra-wide soundstage).
- **Channel Balance & Preamp Gain**: Master left/right panning control and global pre-amplification trim (`-20 dB` to `+20 dB`).

#### 🔬 Tab 3: Advanced Parametric Equalizer & Resampler Quality
- Provides a full 10-band precision biquad filter editor where each individual band can be independently configured:
  - **Filter Shapes**: `Peaking`, `Low Shelf`, `High Shelf`, `Low Pass`, `High Pass`, `Bandpass`, and `Notch` filters (`biquad.rs`).
  - **Quality Factor (`Q`)**: Fully adjustable bandwidth control ranging from `Q = 0.1` (broad gentle slope) to `Q = 24.0` (surgical notch filter for isolating feedback or room modes).
  - **Exact Center Frequency**: Adjustable from `20 Hz` to `20,000 Hz`.
- **Resampler Quality Selector**: Allows users to dynamically select the CPU vs. accuracy tier of the `rubato` polynomial/Catmull-Rom resampler directly from the UI:
  - `Fast`: Optimized linear interpolation for low-power mobile/laptop profiles.
  - `Balanced`: Standard multi-point polyphase resampling.
  - `High Quality 4x`: 4x oversampling with anti-aliasing windowed sinc filters.
  - `Ultra HD`: Maximum precision asynchronous Catmull-Rom resampling with `-140 dB` stop-band attenuation.

> [!TIP]
> **Context-Aware Reset Behavior:** Pressing the *Reset* button inside any specific tab only resets the parameters of that active tab (`equalizerwindow.cpp:L541-L596`). You can safely reset or flatten your graphic EQ curve without wiping out surgical parametric notch filters or room calibration profiles.

---

### 3. Equalizer Workstation Robustness & Ergonomics (`EqualizerWindow` Deep Dive)
The `EqualizerWindow` (`equalizerwindow.cpp`, `equalizerwindow.h`) is engineered as an independent, floating master workstation rather than a rigid or restrictive dialog:

#### 📐 Floating, Resizable & Movable Frameless Geometry
- Built upon a customized Qt window (`setWindowFlags(Qt::Widget | Qt::FramelessWindowHint)`), the EQ panel features a custom dark header bar that acts as a universal drag handle.
- Users can click and drag the EQ window freely anywhere across single or multi-monitor desktop setups without obstructing the main player window (`MainWindow`).
- **Full Resizability:** Unlike fixed `400x300` popups common in older players, `EqualizerWindow` defaults to a spacious **780x560** canvas (`equalizerwindow.cpp:L21`) and allows smooth manual window resizing down to a compact minimum of **650x480** (`setMinimumSize(650, 480)`) or maximized up to full screen, allowing detailed spline inspection during intricate mastering work.

#### 💾 Persistent Geometry & Parameter State (`QSettings`)
- Every physical and acoustic adjustment made inside `EqualizerWindow` is preserved in real time (`saveSettings()`).
- When the user closes the window (`E` shortcut or `Close Button`) or exits PlayTune entirely, `QSettings` records:
  - Exact `(X, Y)` screen coordinates and `(Width, Height)` geometry dimensions.
  - Active tab selection (`10 Bands`, `Controls`, or `Advanced`).
  - Active preset selection (`Custom`, `Pop`, etc.), global preamp gain (`dB`), tone/spatial positions (`Bass`, `Treble`, `Stereo Width`, `Balance`), and individual parametric filter parameters across all 10 bands.
- Upon relaunching, `loadSettings()` restores the exact visual geometry and DSP state instantly without user intervention.

#### 🧮 Mathematical Robustness & Biquad Cascading Stability
Digital equalizers can easily become unstable or introduce digital clipping if the underlying mathematical filter equations are implemented naively. PlayTune's `biquad.rs` ensures uncompromised acoustic robustness:
- **64-Bit Double Precision State Variables:** While audio samples enter and leave the DSP chain in 32-bit float (`f32`), all intermediate difference equations (`Direct Form II Transposed`) compute biquad coefficients (`a0, a1, a2, b0, b1, b2`) using **double-precision floating-point math (`f64`)**. This completely prevents low-frequency coefficient quantization noise when boosting bass frequencies below `100 Hz`.
- **Infinite Oscillations & Clipping Prevention:** At extreme quality factors (`Q = 24.0`), standard recursive biquad filters can suffer from ringing and infinite gain oscillation if filter poles approach the unit circle. PlayTune mathematically clamps filter pole radii and applies automatic coefficient gain normalization, ensuring stable, oscillation-free performance across every frequency band.

---

### 4. Real-Time Partitioned Convolution Engine (`convolution.rs`)
PlayTune includes a native **Partitioned Convolution Engine** capable of loading external `.wav` Impulse Response (`IR`) files.
- **Zero-Latency Partitioning:** Standard frequency-domain convolution (`FFT -> Multiply -> IFFT`) introduces latency equal to the block size of the IR buffer (which can be several seconds for large cathedral or concert hall impulse responses). PlayTune splits the impulse response into small, non-uniform time-domain and frequency-domain partitions (using block sizes as low as `64 samples` for the initial head block).
- **Use Cases:**
  - **Acoustic Room Modeling:** Load famous concert hall, recording studio, or cathedral acoustic IRs to experience spatial realism.
  - **Headphone EQ Calibration:** Load `.wav` calibration impulse responses from **AutoEQ** or **Oratory1990** to flatten the frequency response of specific audiophile headphones with zero phase distortion.
  - **Guitar & Cabinet Simulation:** Can directly process acoustic or electric instruments through high-fidelity speaker cabinet IRs.

---

### 5. Binaural Spatial Crossfeed Processor (`crossfeed.rs`)
When listening to stereo music over headphones, hard left/right separation can cause acoustic unnaturalness and mental fatigue (known as the "in-head localization problem"), since in a real room both ears hear both speakers with slight time delays (`ITD`) and acoustic shadowing (`ILD`).
PlayTune implements three industry-standard binaural crossfeed models inside `crossfeed.rs`:
1. **Chu-Moy Model:** A gentle resistive crossfeed circuit simulation that blends low-to-mid frequencies between left and right channels with a subtle high-frequency bypass (`-6 dB` crossover at `700 Hz`).
2. **Jan Meier Model:** A natural, frequency-dependent crossfeed with delay simulation that preserves transient crispness while providing a cohesive, forward-facing virtual soundstage.
3. **Linkwitz Model:** An audiophile-grade crossfeed designed by Siegfried Linkwitz, utilizing inter-aural time delay (`300 µs`) alongside accurate dipole acoustic shadowing curves (`1.4 kHz` shelf).

---

### 6. Lookahead Peak Limiter & Word-Length Dithering (`limiter.rs`, `dither.rs`)
When applying heavy positive equalization boosts, room convolution, or ReplayGain adjustments, digital signals can exceed `0 dBFS`, causing harsh inter-sample clipping and digital distortion inside DAC converters.
- **Lookahead Brickwall Limiter (`limiter.rs`):** The audio stream passes through a configurable lookahead delay buffer (`2ms - 5ms`). The limiter scans ahead for transients approaching `0 dBFS` and smoothly applies transparent gain attenuation *before* the peak hits the output DAC, preventing digital clipping without introducing pumping or breathing artifacts.
- **High-Fidelity Dithering (`dither.rs`):** When outputting 64-bit/32-bit floating-point DSP pipelines to integer hardware DACs (`16-bit` or `24-bit PCM`), truncating sample words introduces harmonic quantization distortion. PlayTune applies **Triangular Probability Density Function (`TPDF`) dithering** coupled with psychoacoustic **Noise Shaping**, shifting quantization noise floor energy up into the ultra-high frequency spectrum (`18 kHz - 22 kHz`) where the human ear is least sensitive.

---

### 7. Multi-Band Dynamics Compressor (`multiband_compressor.rs`)
For dynamic range control or mastering simulation, PlayTune includes a multi-band compressor that divides the audio spectrum into distinct frequency bands (`Low`, `Mid`, `High`) using phase-accurate Linkwitz-Riley crossover filters.
- Each individual band features independent controls for **Threshold (`dB`)**, **Ratio (`1:1 to 20:1`)**, **Attack Time (`ms`)**, **Release Time (`ms`)**, and **Make-Up Gain (`dB`)**.
- Allows users to tame booming bass frequencies or harsh sibilant treble peaks (`de-essing`) dynamically without compressing the entire track's volume.

---

## 🎚️ Bit-Perfect Audio Drivers & Device Hot-Swapping (`output/cpal_output.rs`)

To ensure pristine, unadulterated audio playback, PlayTune offers full control over how audio samples are delivered to physical hardware audio interfaces:

### 1. Bit-Perfect Hardware Backends
By default, operating systems route all application audio through shared software mixers (`Windows Audio Session API Shared`, `PulseAudio/PipeWire`, `CoreAudio Shared`). These mixers force internal resampling to a single global OS sample rate (`48 kHz`), apply digital volume scaling, and mix audio with system notification bells. PlayTune provides direct hardware exclusive drivers:
- **WASAPI Exclusive Mode (Windows):** Locks the physical USB DAC or sound card directly to PlayTune. Bypasses the Windows OS mixer, disabling system sounds and delivering raw, bit-perfect `PCM` samples exactly matching the track's native sample rate (`44.1 kHz`, `96 kHz`, `192 kHz`).
- **ASIO (Windows Studio Interfaces):** Direct support for Audio Stream Input/Output (`ASIO`) drivers (`Steinberg ASIO`), providing ultra-low latency hardware buffer communication for studio interfaces like RME, Focusrite, and Universal Audio.
- **Direct ALSA Hardware Bypass (Linux):** Connects directly to hardware ALSA PCM devices (`hw:0,0`), bypassing PulseAudio or PipeWire software layers for pure Linux bit-perfect streaming.
- **CoreAudio Hog Mode (macOS):** Acquires exclusive control (`Hog Mode`) over macOS audio hardware devices, preventing other system apps from altering sample rates or injecting audio.

```
+-------------------------------------------------------------------------+
|                    BIT-PERFECT OUTPUT DRIVER SUITE                      |
|                                                                         |
|  +------------------------+  +---------------------------------------+  |
|  | WASAPI EXCLUSIVE (Win) |  | Bypasses Windows Audio Mixer completely |  |
|  +------------------------+  +---------------------------------------+  |
|  +------------------------+  +---------------------------------------+  |
|  | ASIO DIRECT (Windows)  |  | Low-latency Steinberg hardware bridge   |  |
|  +------------------------+  +---------------------------------------+  |
|  +------------------------+  +---------------------------------------+  |
|  | DIRECT ALSA (Linux)    |  | Direct hw:0,0 bypass of Pulse/PipeWire  |  |
|  +------------------------+  +---------------------------------------+  |
|  +------------------------+  +---------------------------------------+  |
|  | COREAUDIO HOG (macOS)  |  | Exclusive hardware lock on Apple DACs   |  |
|  +------------------------+  +---------------------------------------+  |
+-------------------------------------------------------------------------+
```

### 2. Automatic Fallback & Resilient Recovery (`cpal_output.rs`, `tc-device-monitor`)
- **Graceful Shared Fallback:** If a user requests an Exclusive Mode driver or sample rate (`192 kHz`) that the currently connected hardware DAC does not support or if another application currently holds an exclusive lock, PlayTune catches the initialization error and seamlessly falls back to `Auto` (shared mode) (`cpal_output.rs:L121`, `L335`) without crashing or aborting playback.
- **Active Device Hot-Swapping (`tc-device-monitor`):** A dedicated background monitoring thread (`tc-device-monitor`) polls the OS default audio endpoint configuration every `5 seconds`. If a user disconnects Bluetooth headphones (`Sony WH-1000XM5`) or plugs in an external USB DAC (`AudioQuest DragonFly`), the engine detects the device shift, pauses stream execution, renegotiates the optimal sample rate and channel map, and resumes playback seamlessly within `5ms` (`README.md:L86`).

---

## 🔊 ReplayGain & Loudness Normalization (`loudness.rs`, `loudness_scanner.rs`, `LoudnessScannerDialog`)

PlayTune features a complete, professional-grade loudness normalization and ReplayGain 2.0 processing pipeline designed to eliminate volume jumps between tracks:

### 1. EBU R128 & ReplayGain 2.0 Target Scaling
- Unlike basic peak normalizers that normalize audio to `0 dBFS` (which makes compressed pop tracks sound twice as loud as dynamic acoustic tracks), PlayTune measures **Integrated Loudness (`LUFS`)** according to EBU R128 standards.
- Tracks are automatically scaled in real time (`loudness.rs`) to match target listening levels (`-18 LUFS` for ReplayGain 2.0 or `-23 LUFS` for broadcast standard EBU R128).
- **Track Gain vs. Album Gain Mode:**
  - *Track Gain Mode:* Scales every track individually to the target LUFS. Ideal for shuffled playlists across diverse genres.
  - *Album Gain Mode:* Applies a single uniform loudness shift calculated across the entire album. This preserves the artist's intended dynamic contrast between quiet acoustic interlude tracks and explosive climax tracks on concept albums (`Pink Floyd - The Dark Side of the Moon`).
- **True Peak Limiting:** If applying positive ReplayGain boost causes the track's inter-sample true peak (`REPLAYGAIN_TRACK_PEAK`) to exceed `-0.2 dBFS`, the engine automatically clamps the gain curve to guarantee zero digital clipping.

### 2. Built-in Multi-Threaded Loudness Scanner & Tag Writer (`LoudnessScannerDialog`)
PlayTune does not just read external ReplayGain tags—it includes a built-in **Loudness Scanner & Writer Workstation**:
- Users can select entire folders, albums, or playlists and launch the `LoudnessScannerDialog`.
- The scanner spins up background worker threads (`loudness_scanner.rs`) to rapidly decode audio files at maximum speed (`symphonia`), calculating exact values for:
  - `REPLAYGAIN_TRACK_GAIN` (`dB`) & `REPLAYGAIN_TRACK_PEAK` (`dBFS`)
  - `REPLAYGAIN_ALBUM_GAIN` (`dB`) & `REPLAYGAIN_ALBUM_PEAK` (`dBFS`)
  - **Loudness Range (`LRA`)** (`LU` - Loudness Units) measuring the macro-dynamic span of the track.
- With one click, the scanner writes these standardized ID3v2/Vorbis tags directly back to the physical audio files on disk while synchronizing the values into the SQLite WAL database index.

---

## 🎧 Pure-Rust Multi-Format Decoding (`symphonia_decoder.rs`) & File Format Matrix

Most desktop media players rely on external C-libraries like `ffmpeg`, `libavcodec`, or `gstreamer` to decode compressed audio formats. These external C dependencies can introduce memory vulnerabilities, buffer overflows, and complex platform build dependencies.

PlayTune implements **100% Pure Memory-Safe Rust Decoding** via `symphonia` (`symphonia_decoder.rs`), ensuring zero vulnerability to legacy C buffer overflows and guaranteeing sample-accurate **gapless playback transitions**.

### Comprehensive File Format & Codec Specification Matrix

| Container / Extension | Codec Engine | Supported Sample Rates | Bit Depths | Tagging Standards | Gapless Playback |
| :--- | :--- | :--- | :--- | :--- | :---: |
| **`.flac`** | FLAC (Free Lossless Audio Codec) | `8 kHz` to `384 kHz` | 16-bit, 24-bit, 32-bit PCM | **Vorbis Comments** + Embedded Cover Art | ✅ Yes (Exact sample count) |
| **`.wav` / `.wave`** | Linear PCM / IEEE 754 Float / ADPCM | `8 kHz` to `768 kHz` | 16, 24, 32-bit int, 32/64-bit float | **ID3v2.3 / ID3v2.4** in `id3 ` chunk + RIFF INFO | ✅ Yes (Native raw PCM) |
| **`.mp3`** | MPEG-1/2 Audio Layer III | `16 kHz` to `48 kHz` | 16-bit PCM output | **ID3v1, ID3v2.3, ID3v2.4** + LAME Tag headers | ✅ Yes (via LAME delay/padding calculation) |
| **`.m4a` / `.mp4`** | AAC (Advanced Audio Coding LC/HE/HE-v2) | `8 kHz` to `96 kHz` | 16-bit, 24-bit PCM output | **MP4 / iTunes Atom Metadata** (`moov`/`udta`) | ✅ Yes (via `stts` / `edit list` atoms) |
| **`.m4a` (ALAC)** | Apple Lossless Audio Codec | `44.1 kHz` to `192 kHz` | 16-bit, 24-bit, 32-bit PCM | **MP4 / iTunes Atom Metadata** | ✅ Yes (Native lossless) |
| **`.ogg` / `.oga`** | OGG Vorbis | `8 kHz` to `192 kHz` | 16-bit, 24-bit PCM output | **Vorbis Comments** + `METADATA_BLOCK_PICTURE` | ✅ Yes (Sample-accurate granule positions) |
| **`.opus`** | Opus Interactive Audio Codec | `8 kHz` to `48 kHz` (Resampled up to 192kHz) | 16-bit, 24-bit PCM output | **Vorbis Comments** in OpusHead/Tags | ✅ Yes (Pre-skip sample trimming) |
| **`.aiff` / `.aif`** | Audio Interchange File Format (PCM) | `8 kHz` to `384 kHz` | 16-bit, 24-bit, 32-bit PCM | **ID3v2** chunk inside IFF container | ✅ Yes (Native uncompressed PCM) |

---

## 📁 Library Management, Tag Editor & Concurrent SQLite WAL Engine (`library`, `db`, `TagEditorDialog`)

PlayTune is engineered to handle massive local music collections (`100,000+ tracks`) smoothly without UI freezes or database lock contention:

### 1. Concurrent SQLite WAL Database Engine (`modules/db/src/database.rs`)
- **Write-Ahead Logging (`WAL`) Mode:** The SQLite engine is configured in lock-free WAL mode (`pragma journal_mode = WAL; pragma synchronous = NORMAL;`). This allows background library scanning threads to perform heavy disk writes (`INSERT / UPDATE`) simultaneously while the user actively searches, queues tracks, and streams audio from the database on the main thread with **zero lock contention**.
- **Batch Transactional Indexing:** During folder scanning (`modules/library/src/lib.rs`), single-row inserts are strictly prohibited (`README.md Invariant 2`). Scanners group metadata extraction into atomic SQL batch transactions of at least `100 items`, indexing thousands of files per second.

### 2. Single-Pass Walkdir Metadata & Cover Art Extraction (`modules/library/src`)
- When importing music directories, `walkdir` traverses the filesystem recursively.
- For each file, the engine performs a **unified single-pass disk read** (`metadata.rs`, `cover_art.rs`), extracting ID3v1, ID3v2.3, ID3v2.4, Vorbis Comments, and MP4 atoms while simultaneously extracting and decoding embedded album cover art (`JPEG/PNG` via the `image` crate).
- Extracted cover art is automatically resized, cached in memory, and stored alongside the track index for instantaneous grid rendering.

### 3. Built-in Interactive Metadata Tag Editor (`TagEditorDialog`)
PlayTune includes a native Qt6 **Metadata Tag Editor Dialog** (`tageditordialog.cpp`, `tag_editor.rs`):
- Allows users to inspect and edit metadata tags across individual files or batch selections.
- **Editable Fields:** Track Title, Artist, Album, Album Artist, Genre, Release Year, Track Number, Disc Number, and Composer/Comment fields.
- **Direct File Persistence:** Modifications made in `TagEditorDialog` are immediately written back to the physical ID3v2/Vorbis tags on disk while atomically updating the active SQLite database row (`UPDATE songs SET title = ? ... WHERE path = ?`).

---

## 🎤 Synchronized Lyrics Viewer & LRC Parser (`karaokedialog.cpp`, `lrcparser.cpp`)

To deliver an immersive vocal experience, PlayTune features a full-featured synchronized lyrics suite:
- **Universal `LrcParser` Engine:** Automatically scans both external `.lrc` timestamped lyric files residing next to audio files (`song.mp3` + `song.lrc`) and internal embedded lyrics tags (`ID3 USLT` / `SYLT` and `Vorbis LYRICS` fields).
- **Dedicated Karaoke Workstation (`KaraokeDialog`):**
  - Renders high-contrast, large-format synchronized lyrics with smooth vertical scrolling.
  - As the audio ticker advances (`30ms cadence`), the active lyric line is dynamically highlighted with vibrant gradient illumination (`#8A2BE2` to `#00F2FE`) while upcoming lines fade into dark glassmorphism transparency.
  - Supports manual seek-by-lyric: clicking any timestamped line inside the Karaoke dialog instantly jumps the audio engine (`seek_to()`) to that exact vocal entry point.

---

## 🖥️ Native Qt6 C++ GUI, FFT Visualizer & Ergonomic Workspaces (`gui`, `analysis`)

PlayTune's interface is crafted in **Qt6 C++** to provide an ultra-responsive, visually stunning audiophile experience:

### 1. Live 65-Bin Real-Time FFT Spectrum Visualizer (`realfft`, `NowPlayingCard`)
- Unlike web players that fake frequency bars using randomized CSS animations, PlayTune computes a real-time **65-Bin Fast Fourier Transform (`FFT`)** (`modules/analysis/src/lib.rs`) powered by the `realfft` crate.
- **Zero UI-Thread Stalling:** Audio samples are tapped inside the audio engine and processed on the dedicated `Ticker/Analysis Thread` (`30ms cadence`). The calculated logarithmic frequency magnitude bars (`31 Hz` to `16 kHz`) are written into atomic ring buffers (`ArcSwap`) and read cleanly by the Qt6 canvas (`NowPlayingCard::paintEvent`).
- **Idle Waveform Simulation:** When playback is paused or stopped, the visualizer gracefully transitions to a gentle, organic breathing waveform mock animation (`README.md:L96`), ensuring the UI always feels alive and dynamic.

```
65-Bin Real-Time Logarithmic Frequency Spectrum Tap (31 Hz - 16 kHz)
+-----------------------------------------------------------------------------------+
|  |||                                                                              |
|  |||   |||                                 |||                                    |
|  |||   |||   |||                           |||   |||                              |
|  |||   |||   |||   |||         |||         |||   |||   |||                        |
|  |||   |||   |||   |||   |||   |||   |||   |||   |||   |||   |||         |||      |
|  |||   |||   |||   |||   |||   |||   |||   |||   |||   |||   |||   |||   |||   |||  |
+-----------------------------------------------------------------------------------+
  31Hz  63Hz  125Hz 250Hz 500Hz  1kHz  2kHz  4kHz  8kHz 12kHz 16kHz [RealFFT Ring Tap]
```

### 2. Dark-Themed Glass Aesthetics & Custom Tooltip Engine
- **Vibrant Curated Palette:** Built upon rich dark-mode glassmorphism styling (`#060B14` backgrounds, `#8A2BE2` electric violet accents, and `#00F2FE` cyan highlights) (`README.md:L5-L7`).
- **App-Wide Styled Tooltips:** Every slider, knob, and interactive widget app-wide features custom dark-themed overlay tooltips (`custom_widgets.cpp`) showing exact numerical readouts (e.g., `Preamp: +3.2 dB`, `Band 1kHz: -1.5 dB`, `Stereo Width: 140%`).
- **Global Tooltip Filter Toggle:** Users can instantly enable or disable all tooltips app-wide from the Settings Page (`SettingsPage::onToggleTooltips`), which dynamically installs or removes a `QEvent::ToolTip` filter on the Qt `QApplication` instance.

### 3. Ergonomic Sidebar Navigation
- **Sidebar Suite (`sidebar.cpp`):** Rapid, single-click navigation between dedicated workspaces:
  - `Songs Table`: Full tracklist view (`songstable.cpp`) with sorting by Title, Artist, Album, Duration, Bitrate, and Sample Rate.
  - `Folders View`: Direct filesystem directory tree browser (`foldersview.cpp`) for users who prefer folder-structure navigation over tag categorization.
  - `Favorites`: Quick-filter view for starred tracks (`SELECT * FROM songs WHERE is_favorite = 1`).
  - `Recently Played` & `Most Played`: Smart dynamic playlists driven by SQLite play statistics tracking (`last_played_at`, `play_count`).
  - `Settings`: Global player configuration, theme switching, and hardware audio driver selection.

---

## 🌐 OS Desktop & Media Key Integration (`platform`, `souvlaki`)

PlayTune integrates deeply into the native desktop environment of Linux, Windows, and macOS via the `souvlaki` (`modules/platform`) bridge (`README.md:L130-L139`):

| Operating System | Native System Bridge | Supported Media Actions & Integrations |
| :--- | :--- | :--- |
| 🐧 **Linux** | **MPRIS D-Bus Service** (`Media Player Remote Interfacing Specification`) | Broadcasts real-time track metadata (Title, Artist, Album, Art URI, Duration, Position) directly to KDE Plasma / GNOME shell notification banners and system tray applets. Handles hardware media keys (`XF86AudioPlay`, `XF86AudioNext`, `XF86AudioPrev`, `XF86AudioStop`). |
| 🪟 **Windows** | **SystemMediaTransportControls (`SMTC`)** | Synchronizes with the native Windows lock screen media banner and Windows 10/11 taskbar volume/media control overlay popup. |
| 🍎 **macOS** | **MPRemoteCommandCenter** | Native Touch Bar playback controls, macOS Control Center synchronization, and Apple keyboard hardware media keys. |

---

## 🏗️ Refactored Modular Architecture (`src/main.rs`, `handlers/`, `app_state.rs`)

As PlayTune evolved, the original entry file (`main.rs`) expanded into a monolithic `2081 LOC` structure. To maintain strict code organization and separation of concerns (`Conversation ac37ddf8`), the root `src/` crate was systematically refactored into a clean, decoupled modular architecture:

```
src/
├── main.rs            -> Thin entry point (~23.5 KB): Initializes once statics, Qt6 FFI bridges, and thread pools
├── app_state.rs       -> Global state container (`AppState`) synchronizing ArcSwap<PlaybackInfo> and channels
├── bridge.rs          -> C/C++ FFI export layer (`extern "C"`) allowing Qt6 widgets to safely query Rust state
├── ui_sync.rs         -> 30ms Ticker orchestration syncing audio frame advancement, FFT bars, and UI progress
└── handlers/          -> Modular command dispatchers:
    ├── playback.rs    -> Handles Play, Pause, Stop, Seek, Next, and Previous command messages
    ├── library.rs     -> Handles async directory scanning, metadata indexing, and DB query requests
    └── settings.rs    -> Handles EQ preset loads, driver hot-swapping, and configuration persistence
```

### Multithreaded Concurrency Invariants
PlayTune runs 5 distinct multithreaded loops that operate independently without blocking the native UI or audio callbacks:

| Thread Name | Cadence / Trigger | Core Responsibility & Synchronization Mechanism |
| :--- | :---: | :--- |
| **Main UI Thread** | Event Loop | Executes the native Qt6 GUI event loop (`QApplication::exec()`). Never runs blocking file I/O or SQLite calls. |
| **Ticker Sync Thread** | `30ms` Loop | Ticks the audio engine status, calculates current playback progress, dispatches 65-bin FFT spectrum bars via `ArcSwap`, and triggers track auto-advance upon detecting end-of-stream. |
| **Device Monitor Thread** | `5s` Polling | `tc-device-monitor` polls OS audio endpoints. Triggers instantaneous CPAL stream hot-swapping if hardware shifts occur. |
| **Media Key Thread** | `200ms` Polling | Polls `souvlaki` OS channels (MPRIS D-Bus / SMTC) for desktop media key events and system tray commands. |
| **Library Scan Thread** | On-Demand Async | Background worker pool executing recursive `walkdir` traversals and `100+ item` SQLite WAL batch insertions. |

---

## 📋 Verification & Performance Invariants Checklist

When contributing code or developing new modules for PlayTune / TuneCraft, engineers must strictly adhere to the project's **Three Golden Invariants** (`README.md:L235-L259`):

> [!CAUTION]
> **Invariant 1 — Zero Allocation on Audio Hot Paths**
> Functions executing inside the CPAL audio callback (`decode_and_process`) or inside any `dsp/` biquad/convolution/resampling loop **MUST NEVER allocate heap memory** (`Vec::new()`, `Box::new()`, `String::clone()`). All scratch vectors and ring buffers must be pre-allocated during pipeline construction (`Pipeline::new()`).

> [!IMPORTANT]
> **Invariant 2 — Batch I/O for Database Operations**
> Single-row inserts inside directory scanning loops are strictly forbidden. All directory traversals in `library/src` must group database writes into atomic SQLite transactions of **at least 100 items** to maintain sub-millisecond responsiveness on concurrent reading threads.

> [!TIP]
> **Invariant 3 — No Production `unwrap()`**
> Errors across all Rust modules must be propagated using `anyhow::Result` or logged via structured warnings (`log::warn!`). Calling `.unwrap()` or `.expect()` is restricted exclusively to `#[cfg(test)]` unit and integration test blocks.

### Optimized Cargo Release Configuration (`Cargo.toml`)
To guarantee sub-millisecond audio frame delivery and maximum Catmull-Rom resampling throughput, production binaries must always be compiled under the optimized release profile (`cargo run --release`):
```toml
[profile.release]
lto = true          # Full Link-Time Optimization across all 7 workspace crates
strip = true        # Strip debug symbols for compact native binary delivery
codegen-units = 1   # Single codegen unit for maximum function inlining across DSP loops
panic = "unwind"    # Enables safe catching of unwinds inside C/FFI callbacks (std::panic::catch_unwind)
```

---

## 🚀 Future Improvements & Strategic Roadmap

While PlayTune / TuneCraft already stands as a premier audiophile desktop player, development continues actively. Below is the strategic roadmap outlining planned architectural expansions and advanced feature additions:

### 1. VST3 & Audio Unit (AU) Plugin Host Bridge
- **Goal:** Allow users to insert external third-party professional mastering plugins directly into the Rust DSP chain (`pipeline.rs`).
- **Implementation:** Integrating a lightweight VST3/AU host wrapper inside `modules/engine/src/dsp/plugin_host.rs`, allowing users to run studio-grade tools such as *FabFilter Pro-Q 3*, *iZotope Ozone*, *Sooth 2*, and hardware modeling compressors directly in their music playback pipeline.

### 2. UPnP / DLNA & AirPlay 2 Network Streaming
- **Goal:** Extend high-fidelity audio delivery beyond locally connected USB DACs to networked audiophile equipment.
- **Implementation:** Building an asynchronous network output backend inside `modules/engine/src/output/network.rs` supporting UPnP/AV MediaRenderer endpoints and Apple AirPlay 2 streaming protocols, broadcasting bit-perfect FLAC/PCM over home Wi-Fi networks to high-end streamers (*Bluesound Node*, *HiFi Rose*, *Naim Uniti*).

### 3. Automated Web Lyrics Fetcher (`LRCLIB` & `MusicBrainz`)
- **Goal:** Eliminate the need for users to manually download and place `.lrc` files next to audio tracks.
- **Implementation:** Adding a background network service (`modules/library/src/lyrics_fetcher.rs`) that queries open synchronized lyrics APIs (*LRCLIB.net*, *NetEase*, *MusicBrainz*) during library scanning or track playback, automatically downloading and embedding synchronized `.lrc` timestamps directly into local audio tags (`USLT`/`SYLT`).

### 4. Advanced Acoustic Visualizer Suite (Spectrogram & Vector Scope)
- **Goal:** Expand visual analysis tools beyond the current 65-bin FFT bar graph.
- **Implementation:** Utilizing the existing `realfft` backend to render:
  - **3D Waterfall Spectrogram:** A scrolling, color-mapped frequency/time heat map (`20 Hz - 20 kHz`) showing harmonic decay over time.
  - **Lissajous Phase / Vector Scope:** Real-time X/Y stereo phase correlation meter (`Left + Right vs Left - Right`) allowing mastering engineers and audiophiles to detect phase cancellation and verify stereo imaging width.

### 5. Secure CDDA Audio CD Ripping & Transcoding Workstation
- **Goal:** Provide a comprehensive one-stop station for ripping physical optical discs into pristine digital libraries.
- **Implementation:** Adding native CDDA (`Compact Disc Digital Audio`) reading capabilities via `libcdio` / `paranoia`, coupled with **AccurateRip** online checksum verification to guarantee bit-perfect optical reads. Includes a multi-core batch transcoding engine for converting ripped PCM streams directly to FLAC, Opus, or MP3.

### 6. Dynamic HSL Theme Builder & Custom UI Skinning
- **Goal:** Allow complete visual personalization beyond the default dark glassmorphism presets.
- **Implementation:** Creating a live **Theme Builder Workstation** inside the Settings page where users can adjust HSL color wheels (`Accent Color`, `Background Tint`, `Text Contrast`), configure custom Google fonts (`Inter`, `JetBrains Mono`, `Outfit`), and fine-tune acrylic/glass blur radii (`QGraphicsBlurEffect`), exporting custom themes as shareable `.ptskin` JSON packages.

---

*Document compiled from full workspace analysis, architectural refactoring logs (`ac37ddf8`), and feature engineering evaluations (`4e839968`). PlayTune / TuneCraft is open-source software licensed under the **Apache-2.0 License**.*

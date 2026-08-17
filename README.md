<div align="center">

<img src="assets/logo.png" width="140" alt="PlayTune Logo"/>

<img src="https://capsule-render.vercel.app/api?type=waving&color=gradient&customColorList=6,11,20&height=220&section=header&text=PlayTune&fontSize=72&fontColor=ffffff&animation=fadeIn&fontAlignY=38&desc=Audiophile%20Fidelity%20%C2%B7%20Native%20C%2B%2B%20Speed%20%C2%B7%20Zero-Allocation%20Rust%20DSP&descAlignY=58&descSize=17" width="100%"/>

<img src="https://readme-typing-svg.demolab.com?font=Fira+Code&weight=600&size=21&duration=2800&pause=800&color=8A2BE2&center=true&vCenter=true&width=750&lines=Lock-Free+Rust+DSP+Pipeline;Bit-Perfect+WASAPI+Exclusive+%7C+ASIO+%7C+ALSA;Zero-Allocation+Audio+Engine;Instant-Switching+Qt6+Native+GUI;Parallel+Rayon+Library+Scanner;Sub-Millisecond+Frame+Delivery" alt="Typing SVG" />

<br/>

[![Rust Version](https://img.shields.io/badge/Rust-1.85%2B-FF4B4B.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Qt6 Native GUI](https://img.shields.io/badge/Qt6-Native_GUI-41CD52.svg?style=for-the-badge&logo=qt&logoColor=white)](https://www.qt.io/)
[![DSP Engine](https://img.shields.io/badge/DSP-Zero_Allocation_Hot_Path-8A2BE2.svg?style=for-the-badge)](#)
[![Drivers](https://img.shields.io/badge/Output-WASAPI_Exclusive_%7C_ASIO_%7C_ALSA_%7C_Hog-00F2FE.svg?style=for-the-badge)](#)
[![Decoder](https://img.shields.io/badge/Symphonia-0.6.1_Pure_Rust-FF9900.svg?style=for-the-badge)](#)
[![License](https://img.shields.io/badge/License-Apache--2.0-0052FF.svg?style=for-the-badge)](LICENSE)

<br/>

<p>
  <a href="#-why-playtune"><b>Why PlayTune</b></a> •
  <a href="#-visual-interface"><b>Visual Interface</b></a> •
  <a href="#-core-feature-highlights"><b>Features</b></a> •
  <a href="#-audiophile-dsp-workstation"><b>DSP Suite</b></a> •
  <a href="#-architecture--modular-design"><b>Architecture</b></a> •
  <a href="#-workspace-crates"><b>Crates</b></a> •
  <a href="#-getting-started"><b>Quickstart</b></a> •
  <a href="#-keyboard-shortcuts"><b>Shortcuts</b></a> •
  <a href="#-performance-invariants"><b>Invariants</b></a>
</p>

</div>

---

## 🔥 Why PlayTune?

Modern desktop music players often force users to make a painful choice between **fidelity**, **performance**, and **user experience**:
- **Electron/Web Players**: Consume hundreds of megabytes of idle RAM, suffer from garbage collection stutters, and route audio through generic operating system software mixers that resample and alter the audio signal.
- **Legacy Audiophile Players**: Offer sound fidelity but rely on dated UI toolkits, unstable plugin architectures, or sluggish single-threaded library scanners.

**PlayTune bridges this divide without compromise.**

By pairing a **native, high-refresh-rate C++ Qt6 interface** with a **zero-allocation, lock-free Rust audio engine** over an ultra-low latency C/FFI bridge, PlayTune achieves sub-millisecond audio frame delivery, instant startup, silky 60+ FPS UI transitions, and bit-perfect DAC output.

<div align="center">

| 🐢 Typical Electron Player | ⚡ PlayTune v2.3.1 |
| :--- | :--- |
| 300–600 MB idle memory | **~30 MB lightweight memory footprint** |
| Web audio mixer forced resampling | **Bit-perfect direct hardware output (WASAPI Exclusive, ASIO, ALSA `hw:0,0`, CoreAudio Hog)** |
| V8 Garbage collection stutters | **Zero-allocation real-time DSP hot path (`0 heap allocs`)** |
| Sluggish blocking filesystem import | **Parallel Rayon work-stealing multi-core scanner with progressive UI streaming** |
| Chromium web renderer startup delay | **Instant native binary startup (< 100ms)** |
| Generic external codec libraries | **100% Memory-safe pure-Rust decoding via Symphonia 0.6.1** |

</div>

<div align="center">

`⚡ 0 Heap Allocations on Audio Thread`  ·  `🎚️ 65-Bin Real-Time FFT Visualizer`  ·  `🧵 5 Dedicated Background Threads`  ·  `🎧 Pure-Rust Multi-Codec Engine`  ·  `🌍 Linux · Windows · macOS`

</div>

> [!NOTE]
> If you can hear the difference between a high-end DAC and standard onboard audio, you will feel the difference between PlayTune and traditional players.

---

## 📸 Visual Interface

<div align="center">

![PlayTune Modern UI](assets/PlayTune.png)

<sub>✨ Sleek Dark & Light Mode Glassmorphism · Real-Time 65-Bin FFT Visualizer · Floating Parametric & Graphic EQ Workstation · Hierarchical Folder Browser · Synced Karaoke Lyrics ✨</sub>

</div>

---

## ⚡ Core Feature Highlights

<table>
<tr>
<td width="50%" valign="top">

### 🎧 Zero-Allocation Audiophile Engine
`modules/engine`

- **Zero-Allocation Real-Time Hot Path** — Resampling (`rubato 0.15`), biquad filtering, crossfading, and dynamics processing execute entirely within pre-allocated scratch and ring buffers. Zero heap allocations on the audio callback.
- **Bit-Perfect Hardware Output** — Direct DAC access via **WASAPI Exclusive** (Windows), **ASIO Direct** (Windows studio gear), **Direct ALSA `hw:0,0`** (Linux), and **CoreAudio Hog Mode** (macOS) with automatic shared-mode fallback.
- **Hardware Hot-Swapping & Recovery** — `tc-device-monitor` polls audio endpoints every `5s`, recovering streams within `5ms` upon USB DAC or Bluetooth reconnects without UI interruptions.
- **Pure-Rust Symphonia 0.6.1 Decoding** — Native, memory-safe decoding for **FLAC**, **WAV/PCM/AIFF**, **MP3** (LAME gapless), **AAC/M4A/MP4**, **ALAC**, **OGG Vorbis**, and **OPUS**.
- **Anti-Distortion Headroom Guard** — Exponential soft-knee saturation (`soft_clip_sample`) + `-0.3 dBFS` musical headroom limiter prevent tearing and clipping under aggressive bass boosts.

</td>
<td width="50%" valign="top">

### 🖥️ Native Qt6 C++ Interface & Visuals
`modules/gui`

- **Live 65-Bin FFT Spectrum Visualizer** — Powered by `realfft`, computed on the background ticker thread (`30ms cadence`) with zero UI stalls and an organic idle breathing waveform.
- **Instant Theme Switching (< 0.001 ms)** — Pre-compiled static QSS caching (`s_darkQss`, `s_lightQss`) for pure Dark and Light modes with zero stylesheet thrashing.
- **Cursor Follows Playback** — Automatically scrolls and centers the active playing song upon track progression or tab navigation.
- **Ergonomic Workspaces** — Instant access to Songs Table (multi-column sorting), Albums Grid, Artists Grid, Media Grid, and Hierarchical Folder Browser (`FoldersView`).
- **Interactive Slide-Out Queue Drawer** — Drag-and-drop reordering with persistent track IDs, thumbnail artwork, and "Play Next" context menus.
- **App-Wide Tooltip Engine** — Rich dark overlays displaying exact parameter values (`+3.2 dB`, `140% Width`) with global toggle in Settings (`QEvent::ToolTip`).

</td>
</tr>
<tr>
<td width="50%" valign="top">

### 🗃️ Parallel Scanner & Concurrent SQLite WAL
`modules/library` + `modules/db`

- **Parallel Rayon Multi-Core Scanner** — High-throughput work-stealing parallel iterator pool (`par_iter`) utilizing all CPU cores with `mtime` fast pre-filtering to index thousands of tracks per second.
- **Progressive Real-Time UI Streaming** — Discovered tracks stream directly into the UI tables and queue during folder scanning without waiting for full directory scans to finish.
- **Lock-Free SQLite WAL Database** — Write-Ahead Logging (`WAL`) allows concurrent background metadata indexing and tag editing while you search, queue, and stream without database locks.
- **Single-Pass Tag & Artwork Extraction** — Recursive disk traversals pull metadata tags (ID3v1/v2, Vorbis, MP4) and decode album artwork (`image` crate) in a single disk pass.

</td>
<td width="50%" valign="top">

### 🛠️ Built-in Audiophile Workstations
`modules/gui` + `modules/library`

- **Interactive Metadata Tag Editor (`TagEditorDialog`)** — Inspect and edit ID3v1/v2, Vorbis, and MP4 tags (Title, Artist, Album, Genre, Year, Track/Disc #) directly to files and database atomically.
- **Synchronized Lyrics & Karaoke (`KaraokeDialog`)** — `LrcParser` reads external `.lrc` files and embedded tags (`USLT`/`SYLT`), featuring gradient vocal highlighting and click-to-seek playback.
- **EBU R128 Loudness Scanner (`LoudnessScannerDialog`)** — Multi-threaded background analysis calculating LUFS, True Peak, and Loudness Range (`LRA`), writing standardized ReplayGain 2.0 tags.
- **Sleep Timer (`SleepTimerDialog`)** — Customizable countdown timer or sleep-after-current-track trigger for late-night listening.

</td>
</tr>
</table>

---

## 🎛️ Audiophile DSP Workstation

PlayTune features a comprehensive, modular digital signal processing graph (`modules/engine/src/dsp`) engineered for total mathematical accuracy and real-time stability:

```
+---------------------------------------------------------------------------------------------------+
|                                  PLAYTUNE REAL-TIME DSP PIPELINE                                  |
+---------------------------------------------------------------------------------------------------+
  [Symphonia Audio Decoder] ➔ (32-bit Float Audio Stream)
            │
            ▼
  [Rubato Resampler] ➔ (Fast | Balanced | High Quality 4x | Ultra HD Catmull-Rom Conversion)
            │
            ▼
  [Gain & Acoustic Protection] ➔ (Preamp Gain Trim -20dB to +20dB | volume.snap() Spike Guard)
            │
            ▼
  [Loudness Normalization] ➔ (EBU R128 / ReplayGain 2.0 Track & Album Scaling | True Peak Clamping)
            │
            ▼
  +─────────────────────────────────────────+─────────────────────────────────────────+
  │                                                                                   │
  ▼ [If Parametric EQ Active]                                                         ▼ [If Graphic EQ Active]
  [10-Band Parametric EQ]                                                             [10-Band Graphic EQ]
  (64-bit Double Precision Math, 7 Filter Shapes,                                     (ISO Bands 31Hz - 16kHz, Catmull-Rom
   0.1 - 24.0 Q Factor, 20Hz - 20kHz Centers)                                          Spline Curves, 7 Genre Presets)
  │                                                                                   │
  +─────────────────────────────────────────+─────────────────────────────────────────+
            │
            ▼
  [Multi-Band Compressor] ➔ (Linkwitz-Riley 3-Band Frequency Crossover | Independent Dynamics)
            │
            ▼
  [Partitioned Convolution Engine] ➔ (Zero-Latency Impulse Response Reverb & Headphone AutoEQ)
            │
            ▼
  [Binaural Spatial Crossfeed] ➔ (Chu-Moy | Jan Meier | Linkwitz Acoustic Headphone Fatigue Reducer)
            │
            ▼
  [Stereo Field Processor] ➔ (Mid/Side Processing | Stereo Width 0% Mono to 200% Super-Wide)
            │
            ▼
  [Crossfader] ➔ (Linear | Constant Power | Logarithmic | S-Curve Seamless Track Transitions)
            │
            ▼
  [Soft-Knee Saturation & Headroom Limiter] ➔ (Anti-Clipping Exponential Saturation | -0.3dBFS Peak Guard)
            │
            ▼
  [TPDF Dithering] ➔ (16/24-Bit Triangular Dithering with Psychoacoustic High-Frequency Noise Shaping)
            │
            ▼
  [Bit-Perfect Output Drivers] ➔ (WASAPI Exclusive | ASIO Direct | Direct ALSA hw:0,0 | CoreAudio Hog)
```

### 🎚️ 3-Tab Segmented Equalizer Workstation (`EqualizerWindow`)

PlayTune houses its acoustics controls inside a floating, frameless, resizable (`650x480` to full screen), draggable workstation window that remembers its exact coordinate geometry via `QSettings`:

| Workstation Tab | Architecture & Capabilities |
| :--- | :--- |
| **🎚️ 10-Band Graphic EQ** | Standard ISO bands (`31 Hz`, `63 Hz`, `125 Hz`, `250 Hz`, `500 Hz`, `1 kHz`, `2 kHz`, `4 kHz`, `8 kHz`, `16 kHz`) interpolated with natural **Catmull-Rom splines** to prevent phase distortion. Includes **7 curated genre presets** (*Flat, Pop, Rock, Jazz, Classical, Electronic, Hip Hop*) plus custom user memory. |
| **🎛️ Tone & Spatial Controls** | Poweramp-tuned **Bass** (`100 Hz`, $Q=1.0$) and **Treble** (`7.5 kHz`, $Q=0.7$) shelf filters, 3D **Stereo Width Expansion** (`0%` mono summing to `200%` Mid/Side expansion), true **Linear Balance**, and master **Preamp Gain** (`-20 dB` to `+20 dB`). |
| **🔬 10-Band Parametric EQ** | Surgical mastering biquad filters computed with **64-bit double precision (`f64`) math** to eliminate low-frequency quantization distortion. Supports 7 filter shapes (*Peaking, Low Shelf, High Shelf, Low Pass, High Pass, Bandpass, Notch*), $Q$ bandwidth from `0.1` to `24.0`, and exact frequency tuning (`20 Hz`–`20 kHz`). Houses the **Resampler Quality Selector** (*Fast, Balanced, High Quality 4x, Ultra HD*). |

> [!TIP]
> **Context-Aware Reset Button:** Clicking *Reset* only clears settings inside the currently active tab, allowing you to flatten graphic EQ curves without losing custom parametric notch filters or spatial calibrations.

<details>
<summary><b>🌊 Real-Time Partitioned Convolution Engine</b> — <code>convolution.rs</code></summary>
<br/>

- **Zero-Latency Partitioned FFT:** Splits impulse responses into non-uniform time-domain and frequency-domain partitions (down to `64 samples` head latency), eliminating the multi-second delay of traditional frequency-domain convolution.
- **Acoustic Room & Studio Reverb:** Load any `.wav` Impulse Response (IR) file to emulate legendary concert halls, live stages, and studio recording environments.
- **Headphone EQ Calibration:** Directly load calibration IRs from **AutoEQ** or **Oratory1990** to achieve reference flat response curves across hundreds of audiophile headphone models with zero phase smear.

</details>

<details>
<summary><b>🎧 Binaural Spatial Crossfeed</b> — <code>crossfeed.rs</code></summary>
<br/>

Listening to stereo masterings on headphones can cause acoustic fatigue due to artificial hard channel separation. PlayTune provides three selectable crossfeed models:
1. **Chu-Moy Profile:** Classic resistive crossfeed circuit blending low-to-mid frequencies with a `-6 dB` crossover at `700 Hz`.
2. **Jan Meier Profile:** Frequency-dependent delay simulation preserving transient crispness while centering the virtual soundstage forward.
3. **Linkwitz Profile:** High-end acoustic model using `300 µs` inter-aural time delay (`ITD`) and dipole acoustic shadowing (`ILD`) at `1.4 kHz`.

</details>

<details>
<summary><b>📊 Multi-Band Dynamics Compressor</b> — <code>multiband_compressor.rs</code></summary>
<br/>

- Divides the frequency spectrum into three bands (*Low, Mid, High*) via phase-aligned Linkwitz-Riley crossover filters.
- Independent per-band controls for **Threshold (`dB`)**, **Ratio (`1:1 to 20:1`)**, **Attack Time (`ms`)**, **Release Time (`ms`)**, and **Make-Up Gain (`dB`)**.
- Enables surgical dynamic control, taming unruly bass resonances or sibilant treble peaks without squashing the overall track dynamics.

</details>

<details>
<summary><b>🧠 Intelligent ML Mood Classifier & Feature Extractor</b> — <code>modules/analysis</code></summary>
<br/>

- **DSP Feature Extraction:** Computes spectral centroid, signal energy, zero-crossing rate (ZCR), harmonic-to-percussive ratio (HPR), loudness, and tempo across tracks using stack-allocated zero-allocation routines.
- **Automated Mood Classification:** Categorizes library tracks into 7 distinct mood states (*Calm*, *Energetic*, *Happy*, *Sad*, *Romantic*, *Party*, *Lofi*) using LightGBM machine learning models (`assets/mood_models.json`).
- **CLI Training Tool:** Export custom training datasets and train personalized mood classification models via `cargo run -- export-training-data` and `python3 tools/train_mood_model.py`.

</details>

---

## 🏗️ Architecture & Modular Design

PlayTune enforces a strict **separation of concerns**: the C++ Qt6 GUI holds zero business logic, interacting with the Rust audio engine and database purely through atomic buffers (`ArcSwap<PlaybackInfo>`), lock-free state structures, and concurrent command channels (`crossbeam`).

<div align="center">

```
+-----------------------------------------------------------------------------------+
|                              C++ Qt6 NATIVE FRONTEND                              |
|   MainWindow | EqualizerWindow | KaraokeDialog | TagEditorDialog | LoudnessDialog  |
|   FoldersView | SongsTable     | Sidebar       | QueueWidget     | MediaGridView   |
+-----------------------------------------------------------------------------------+
       ▲                 │                                   ▲
       │ Lock-Free State │ C/FFI Command Channel             │ 65-Bin RealFFT
       │ (ArcSwap)       │ (Crossbeam Channels)              │ Ring Buffers
       ▼                 ▼                                   │
+-----------------------------------------------------------------------------------+
|                        RUST CORE & HANDLER SUBSYSTEM                              |
|   src/main.rs ➔ app_state.rs ➔ bridge/ (commands, ffi, types) ➔ ui_sync.rs        |
|   handlers/ ➔ playback.rs | eq.rs | nav.rs | library/ (import, tags, search, etc.)|
+-----------------------------------------------------------------------------------+
  │                     │                     │                     │
  ▼                     ▼                     ▼                     ▼
+-----------------+   +-----------------+   +-----------------+   +-----------------+
| modules/engine  |   | modules/db      |   | modules/library |   | modules/platform|
| Symphonia 0.6.1 |   | SQLite WAL Mode |   | Rayon Parallel  |   | Souvlaki Media  |
| Rubato / DSP    |   | Concurrent Read |   | Multi-Core Scan |   | MPRIS / SMTC /  |
| Bit-Perfect Out |   | Batch Inserts   |   | Single-Pass Tag |   | MPRemoteCommand |
+-----------------+   +-----------------+   +-----------------+   +-----------------+
```

</div>

### 🧵 Multithreaded Execution Model

| Thread Name | Cadence / Trigger | Core Responsibility & Synchronization |
| :--- | :---: | :--- |
| **Main UI Thread** | Qt Event Loop | Executes the native Qt6 GUI event loop (`QApplication::exec()`). Never runs blocking file I/O or SQLite operations. |
| **Ticker Sync Thread** | `30ms` Loop | Syncs audio progress, computes 65-bin FFT spectrum bars via `ArcSwap`, updates active playback state, and triggers auto-advance on track completion. |
| **Device Monitor Thread** | `5s` Polling | `tc-device-monitor` polls OS audio endpoints, triggering automatic sample rate renegotiation and 5ms stream hot-swapping on disconnects/reconnects. |
| **Media Key Thread** | `200ms` Polling | Polls `souvlaki` desktop media channels (MPRIS D-Bus / Windows SMTC / macOS MPRemoteCommandCenter) for hardware media key events. |
| **Library Scan Thread** | On-Demand Parallel | Rayon work-stealing worker pool traversing filesystem hierarchies concurrently and streaming batch SQLite transactions to the UI. |

---

## 📦 Workspace Crates

PlayTune is organized into 7 highly specialized, decoupled workspace crates:

<div align="center">

| Crate Name | Path | Core Responsibilities |
| :--- | :--- | :--- |
| **`playtune`** | `src/` | Root binary, modular handlers (`playback`, `eq`, `nav`, `library/`), C/FFI bridge exports, UI ticker loop, global `AppState` |
| **`engine`** | `modules/engine/` | Symphonia 0.6.1 decoding, `rubato` resampling, zero-allocation DSP chain, bit-perfect output drivers (WASAPI/ASIO/ALSA/Hog), device recovery |
| **`gui`** | `modules/gui/` | Native C++ Qt6 interface, custom dark/light theme engine, frameless EQ workstation, karaoke viewer, tag editor, media grids |
| **`db`** | `modules/db/` | Concurrent SQLite WAL database engine, tracks, folders, playlists, albums, artists, audio features, ratings/dislikes |
| **`library`** | `modules/library/` | Rayon parallel directory scanner, single-pass tag & cover art extraction, tag editor backend, loudness scanner, playlist I/O |
| **`platform`** | `modules/platform/` | Native OS media key listener and desktop integration via `souvlaki` (MPRIS D-Bus, Windows SMTC, macOS Touch Bar) |
| **`config`** | `modules/config/` | Persistent JSON preferences, DSP settings, equalizer presets, library paths, window state via `serde` |
| **`analysis`** | `modules/analysis/` | Audio feature extraction (ZCR, spectral centroid, energy, tempo), LightGBM 7-mood classifier, `realfft` utilities |

</div>

---

## 🚀 Getting Started

### 1️⃣ Prerequisites

| Dependency | Minimum Version | Purpose |
| :--- | :--- | :--- |
| **Rust Toolchain** | `1.85+` stable | Core backend compilation (`rustup`) |
| **Qt6 SDK** | `6.2+` | C++ GUI widgets (`qt6-base-dev` or `qt6-qtbase-devel`) |
| **CMake & C++ Compiler** | `3.16+` / C++17 | Required by `build.rs` to compile the Qt6 frontend |
| **ALSA & D-Bus** | — | Linux only (`libasound2-dev` and `libdbus-1-dev`) |

<details>
<summary><b>📥 Package Manager Installation Commands</b></summary>
<br/>

**Debian / Ubuntu / Linux Mint**
```bash
sudo apt update && sudo apt install build-essential cmake pkg-config qt6-base-dev libasound2-dev libdbus-1-dev
```

**Fedora / RHEL**
```bash
sudo dnf install cmake pkg-config gcc-c++ qt6-qtbase-devel alsa-lib-devel dbus-devel
```

**Arch Linux / Manjaro**
```bash
sudo pacman -S base-devel cmake pkgconf qt6-base alsa-lib dbus
```

**Windows**
- Install [Rustup](https://rustup.rs/) (MSVC toolchain).
- Install [Qt 6.2+](https://www.qt.io/download) and [CMake](https://cmake.org/download/).
- Ensure `CMAKE_PREFIX_PATH` points to your Qt6 installation.

**macOS**
```bash
brew install cmake qt@6 pkg-config
```

</details>

### 2️⃣ Build & Launch

```bash
# Clone the repository
git clone https://github.com/Sudipta615/PlayTune-Music-Player.git
cd PlayTune-Music-Player

# Build and run with optimized release settings
cargo run --release
```

> [!IMPORTANT]
> **Always compile with `--release`**: PlayTune's zero-allocation DSP pipeline, Catmull-Rom resamplers, and multi-band biquads rely on aggressive compiler optimizations (`lto = "fat"`, `opt-level = 3`, `codegen-units = 1`). Running unoptimized debug builds may cause audio underruns.

### 3️⃣ Shipped Distribution Binary

To generate the maximum-performance standalone binary:

```bash
cargo build --profile dist
```

### 4️⃣ Logging & Diagnostics

```bash
RUST_LOG=info cargo run --release    # Standard operational logging
RUST_LOG=debug cargo run --release   # Detailed DSP & database tracing
```

---

## ⌨️ Keyboard Shortcuts Reference

| Shortcut | Context | Action |
| :--- | :--- | :--- |
| <kbd>Space</kbd> | Global | **Play / Pause** toggle |
| <kbd>Ctrl</kbd> + <kbd>→</kbd> / <kbd>←</kbd> | Global | **Seek Forward / Backward** by 5 seconds |
| <kbd>Alt</kbd> + <kbd>→</kbd> / <kbd>←</kbd> | Global | **Next / Previous Track** |
| <kbd>Ctrl</kbd> + <kbd>↑</kbd> / <kbd>↓</kbd> | Global | **Volume Up / Down** (in 5% increments) |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | Global | **Add File** to library / queue |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd> | Global | **Add Folder** to library (background indexed) |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | Global / Settings | Focus **Global Search** (or Settings Search) |
| <kbd>E</kbd> | Global | Open / Focus **Equalizer & DSP Workstation** |
| <kbd>L</kbd> | Global | Open **Synchronized Lyrics / Karaoke Dialog** |
| <kbd>T</kbd> | Songs Table | Open **Metadata Tag Editor** for selected track |
| <kbd>S</kbd> | Global | Open **Sleep Timer Dialog** |
| <kbd>Delete</kbd> | Queue Widget | Remove selected item from queue |

---

## 📖 Machine Learning Mood Model Training (Developer Guide)

PlayTune ships with pre-trained LightGBM mood classification models in `assets/mood_models.json`. Developers can train custom models on their own music collections:

1. Open PlayTune and create playlists prefixed with `Mood - ` (e.g. `Mood - Happy`, `Mood - Sad`, `Mood - Energetic`, `Mood - Calm`, `Mood - Romantic`, `Mood - Party`, `Mood - Lofi`).
2. Add reference tracks into their corresponding mood playlists.
3. Export training features to CSV:
   ```bash
   cargo run --release -- export-training-data
   ```
   *Generates `training_dataset.csv` with extracted DSP audio features.*
4. Train the LightGBM models:
   ```bash
   python3 tools/train_mood_model.py training_dataset.csv assets/mood_models.json
   ```
5. Done! The generated `assets/mood_models.json` will be embedded directly into PlayTune for instant, automated classification during library scans.

---

## ⚡ Performance Invariants

Every contribution to PlayTune must respect three foundational architectural invariants:

> [!CAUTION]
> **Invariant 1 — Zero Allocation on Audio Hot Paths**
> Functions executing on the CPAL audio output callback or inside `modules/engine/src/dsp` **must never allocate heap memory** (`Vec::new()`, `Box::new()`, `String::clone()`). All scratch and ring buffers must be pre-allocated during pipeline initialization.

> [!IMPORTANT]
> **Invariant 2 — Batch I/O for Database Operations**
> Single-row inserts inside directory scanning loops are strictly forbidden. All directory scans must group writes into SQLite transactions of at least `100` items to maintain sub-millisecond responsiveness on concurrent readers.

> [!TIP]
> **Invariant 3 — Safe FFI Boundaries & No Production Panic**
> Errors across all Rust modules must be handled via `anyhow::Result` or structured logging. Unwind safety across C/FFI boundaries must be guaranteed with `ffi_safe!` and `panic = "unwind"`. Calling `.unwrap()` or `.expect()` is restricted exclusively to test blocks.

---

## 🤝 Contributing & License

Contributions are warmly welcome! Whether you are optimizing DSP SIMD routines, refining Qt glassmorphism styles, or enhancing platform drivers:

1. Fork the repository and create a feature branch (`git checkout -b feat/my-feature`)
2. Verify code quality and format:
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
3. Submit a Pull Request!

<div align="center">

<a href="https://github.com/Sudipta615/PlayTune-Music-Player/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=Sudipta615/PlayTune-Music-Player" />
</a>

<br/><br/>

PlayTune is open-source software licensed under the **Apache-2.0 License**. See [LICENSE](LICENSE) for details.

<br/>

<img src="https://capsule-render.vercel.app/api?type=waving&color=gradient&customColorList=6,11,20&height=120&section=footer" width="100%"/>

<sub>Built with 🦀 Rust & ⚡ Qt6 C++ — engineered for uncompromising acoustic perfection.</sub>

</div>
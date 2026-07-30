use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crossbeam::queue::ArrayQueue;
use crossbeam::utils::CachePadded;

/// Maximum number of frames in the decode-to-DSP buffer
pub const DECODE_BUFFER_FRAMES: usize = 16384;
/// Maximum number of frames in the DSP-to-output buffer
pub const OUTPUT_BUFFER_FRAMES: usize = 8192;
/// Default sample rate
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;
/// Maximum channels we support
pub const MAX_CHANNELS: usize = 2;

#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    #[error("FixedFrameBuffer capacity must be > 0, got {0}")]
    InvalidCapacity(usize),
    #[error("AudioFrame channel count must be 1 or 2, got {0}")]
    InvalidChannelCount(u8),
}

/// A single audio frame (interleaved, up to MAX_CHANNELS)
#[derive(Debug, Clone, Copy)]
pub struct AudioFrame {
    pub channels: [f32; MAX_CHANNELS],
    pub num_channels: u8,
}

impl AudioFrame {
    #[inline]
    pub fn stereo(left: f32, right: f32) -> Self {
        Self { channels: [left, right], num_channels: 2 }
    }

    /// Create a mono frame. The sample is duplicated to both channels so that
    /// downstream stereo code (output device, stereo pipeline) receives the
    /// correct signal on both L and R instead of silence on the right channel.
    #[inline]
    pub fn mono(sample: f32) -> Self {
        Self { channels: [sample, sample], num_channels: 1 }
    }

    #[inline]
    pub fn zero(num_channels: u8) -> Result<Self, BufferError> {
        if num_channels == 0 || num_channels > MAX_CHANNELS as u8 {
            return Err(BufferError::InvalidChannelCount(num_channels));
        }
        Ok(Self { channels: [0.0; MAX_CHANNELS], num_channels })
    }

    #[inline]
    pub fn zero_stereo() -> Self {
        Self { channels: [0.0; MAX_CHANNELS], num_channels: 2 }
    }

    #[inline]
    pub fn get(&self, channel: usize) -> f32 {
        self.channels.get(channel).copied().unwrap_or(0.0)
    }

    #[inline]
    pub fn set(&mut self, channel: usize, value: f32) {
        if channel < MAX_CHANNELS {
            self.channels[channel] = value;
        }
    }

    /// Scale all channel slots by `gain`.
    #[inline]
    pub fn scale(&mut self, gain: f32) {
        for ch in &mut self.channels {
            *ch *= gain;
        }
    }

    /// Interpolate between two frames.
    ///
    /// When mixing frames of different channel counts (e.g., mono + stereo),
    /// the result is promoted to the larger channel count. The missing channel
    /// in the narrower frame is treated as the value of channel[0] (centre
    /// duplication) rather than 0.0 (silence), which was the previous behaviour.
    /// Using 0.0 caused an abrupt amplitude drop on the wider channel during
    /// crossfades between mono and stereo sources.
    #[inline]
    pub fn lerp(&self, other: &AudioFrame, t: f32) -> AudioFrame {
        let max_ch = self.num_channels.max(other.num_channels) as usize;
        let mut result = *self;
        for i in 0..max_ch {
            // For a narrower frame, repeat channel[0] instead of using 0.0
            // to avoid a silent channel on the wider side of the crossfade.
            let a =
                if i < self.num_channels as usize { self.channels[i] } else { self.channels[0] };
            let b =
                if i < other.num_channels as usize { other.channels[i] } else { other.channels[0] };
            result.channels[i] = a * (1.0 - t) + b * t;
        }
        result.num_channels = max_ch as u8;
        result
    }
}

/// A chunk of audio frames for batch processing
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub frames: Vec<AudioFrame>,
    pub sample_rate: u32,
}

impl AudioChunk {
    pub fn new(sample_rate: u32, capacity: usize) -> Self {
        let mut frames = Vec::with_capacity(capacity);
        frames.resize(capacity, AudioFrame::stereo(0.0, 0.0));
        Self { frames, sample_rate }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn num_channels(&self) -> u8 {
        self.frames.first().map(|f| f.num_channels).unwrap_or(2)
    }
}
// We now wrap `crossbeam::queue::ArrayQueue<AudioFrame>`, which provides
// sound `&self` `push`/`pop`/`len`/`is_empty`/`capacity` methods.
// ─────────────────────────────────────────────────────────────────────────
// PcmRingBuffer: lock-free SPSC ring of interleaved f32 samples.
//
// Designed for the audio hot path: the decode loop (producer) and the cpal
// audio callback (consumer) both push/pop **blocks** of N samples at a time,
// not single frames. This reduces the atomic-operation rate from
//   44 100 CAS/sec (one per sample frame at 44.1 kHz stereo)
// to
//   ~86 load+store pairs/sec (one per audio callback) +
//   ~10 load+store pairs/sec (one per decoded 4096-frame chunk).
//
// That is a ~400x reduction in atomic operations on the real-time audio
// thread, with the additional benefit that each block transfer is a single
// memcpy (often SIMD-vectorized by the compiler) instead of N individual
// slot writes.
/// Lock-free single-producer single-consumer ring buffer of interleaved
/// f32 PCM samples. Designed for the audio hot path between the decode
/// thread (producer) and the cpal audio callback (consumer).
pub struct PcmRingBuffer {
    /// Interleaved sample storage. Length is always a power of two.
    buf: UnsafeCell<Box<[f32]>>,
    /// `buf.len() - 1`. Used as a bitmask for O(1) wrap-around.
    mask: usize,
    /// Total capacity in samples (== `buf.len()`).
    capacity: usize,
    /// Write position (producer-only). Wraps monotonically; the actual
    /// index in `buf` is `head & mask`.
    head: CachePadded<AtomicUsize>,
    /// Read position (consumer-only). Wraps monotonically; the actual
    /// index in `buf` is `tail & mask`.
    tail: CachePadded<AtomicUsize>,
}

impl PcmRingBuffer {
    /// Create a new ring buffer with at least `min_capacity` sample slots.
    /// The actual capacity is rounded up to the next power of two so the
    /// wrap-around can use a bitmask instead of a modulo.
    pub fn new(min_capacity: usize) -> Self {
        let cap = min_capacity.max(2).next_power_of_two();
        Self {
            buf: UnsafeCell::new(vec![0.0f32; cap].into_boxed_slice()),
            mask: cap - 1,
            capacity: cap,
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    /// Number of samples that can be pushed without blocking.
    /// Safe to call from either thread; the result is a snapshot and may
    /// be stale by the time it is used.
    #[inline]
    pub fn free_slots(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        self.capacity - head.wrapping_sub(tail)
    }

    /// Number of samples available to be popped. Safe to call from either
    /// thread; the result is a snapshot and may be stale by the time it
    /// is used.
    #[inline]
    pub fn available(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Relaxed);
        head.wrapping_sub(tail)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Push a block of interleaved samples into the ring buffer.
    /// Returns the number of samples actually written (may be less than
    /// `samples.len()` if the ring is nearly full).
    ///
    /// # Safety contract (SPSC)
    ///
    /// Only ONE thread may call this method. Violating the SPSC invariant
    /// is undefined behavior (concurrent writes to `head` race).
    #[inline]
    pub fn push_block(&self, samples: &[f32]) -> usize {
        if samples.is_empty() {
            return 0;
        }
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let free = self.capacity - head.wrapping_sub(tail);
        let n = samples.len().min(free);
        if n == 0 {
            return 0;
        }
        // Two-segment write because the ring wraps. The first segment goes
        // from `start` to the end of the buffer; the second (if any) wraps
        // around to the beginning.
        //
        // SAFETY: SPSC invariant — only the producer thread calls
        // `push_block`. The consumer cannot read these slots until we
        // publish the new `head` with `Release` below. We mutate `buf`
        // through a raw pointer obtained from the `UnsafeCell`; this is
        // sound because (a) we are the only writer to these slots, and
        // (b) the `Release` store on `head` establishes a happens-before
        // relationship with the consumer's `Acquire` load.
        let start = head & self.mask;
        let first = n.min(self.capacity - start);
        // SAFETY: `start + first <= self.capacity` (because
        // `first <= self.capacity - start`) and `first <= n`.
        unsafe {
            let buf_ptr = self.buf.get();
            let buf_slice = std::slice::from_raw_parts_mut((*buf_ptr).as_mut_ptr(), self.capacity);
            buf_slice[start..start + first].copy_from_slice(&samples[..first]);
            let second = n - first;
            if second > 0 {
                buf_slice[..second].copy_from_slice(&samples[first..n]);
            }
        }
        // Release: the consumer must see the written samples before it sees
        // the updated head.
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Pop a block of interleaved samples from the ring buffer into `out`.
    /// Returns the number of samples actually read (may be less than
    /// `out.len()` if the ring is nearly empty). The unused tail of `out`
    /// is left untouched (caller's responsibility to fill with zeros if
    /// needed).
    ///
    /// # Safety contract (SPSC)
    ///
    /// Only ONE thread may call this method. Violating the SPSC invariant
    /// is undefined behavior (concurrent writes to `tail` race).
    #[inline]
    pub fn pop_block(&self, out: &mut [f32]) -> usize {
        if out.is_empty() {
            return 0;
        }
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        let available = head.wrapping_sub(tail);
        let n = out.len().min(available);
        if n == 0 {
            return 0;
        }
        let start = tail & self.mask;
        let first = n.min(self.capacity - start);
        // SAFETY: SPSC invariant — only the consumer thread calls
        // `pop_block`. The producer cannot write to these slots until we
        // publish the new `tail` with `Release` below. We read `buf`
        // through a raw pointer obtained from the `UnsafeCell`; this is
        // sound because the producer's `Release` store on `head` (which
        // we loaded with `Acquire` above) ensures we see all writes to
        // these slots that happened before the producer advanced `head`.
        unsafe {
            let buf_ptr = self.buf.get();
            let buf_slice = std::slice::from_raw_parts((*buf_ptr).as_ptr(), self.capacity);
            out[..first].copy_from_slice(&buf_slice[start..start + first]);
            let second = n - first;
            if second > 0 {
                out[first..n].copy_from_slice(&buf_slice[..second]);
            }
        }
        // Release: the producer must see the consumed slots before it sees
        // the updated tail.
        self.tail.store(tail.wrapping_add(n), Ordering::Release);
        n
    }

    /// Reset the ring to empty. Only safe to call when the consumer is
    /// paused (i.e. no concurrent `pop_block` is in flight). The producer
    /// may still be active — its next `push_block` will simply observe
    /// `free = capacity` and overwrite the old data.
    ///
    /// This matches the existing usage pattern in `CpalOutput::reset_buffer`
    /// which calls `pause()` before `reset()`.
    pub fn reset(&self) {
        // CAS loop: re-read head each iteration so we always publish the
        // most up-to-date empty-state tail. If the producer keeps pushing
        // after our first load, we'll observe the new head on retry and
        // publish that instead. After at most a couple of iterations the
        // producer either quiesces or we accept the latest observed head.
        // We bound the retry count to avoid an infinite loop in the
        // (theoretically impossible) case of a producer pushing faster
        // than we can read.
        const MAX_RESET_RETRIES: usize = 8;
        for _ in 0..MAX_RESET_RETRIES {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Relaxed);
            if tail == head {
                return;
            }
            // Try to advance tail to head. If another thread (shouldn't
            // happen per the SPSC contract, but be defensive) modified
            // tail, the CAS fails and we retry with a fresh head load.
            if self.tail.compare_exchange(tail, head, Ordering::AcqRel, Ordering::Relaxed).is_ok() {
                return;
            }
        }
        let head = self.head.load(Ordering::Acquire);
        self.tail.store(head, Ordering::Release);
    }
}

// SAFETY: PcmRingBuffer is safe to share between threads because:
//   - `buf` is only read/written through atomic head/tail indices.
//   - The SPSC invariant (one producer, one consumer) is enforced by
//     the call-site — see `FixedFrameBuffer`'s docstring.
//   - `head` and `tail` are atomic, so concurrent reads/writes are sound.
unsafe impl Send for PcmRingBuffer {}
unsafe impl Sync for PcmRingBuffer {}

/// The write-half of the SPSC ring buffer.
///
/// Only one `Producer` may exist per buffer (enforced by `Arc` ownership
/// pattern in `create_fixed_frame_buffer`). Although `ArrayQueue` is sound
/// under MPMC, we preserve this invariant to make the producer/consumer
/// split explicit at the type level.
pub struct Producer {
    inner: Arc<ArrayQueue<AudioFrame>>,
}

/// The read-half of the SPSC ring buffer.
///
/// Only one `Consumer` may exist per buffer.
pub struct Consumer {
    inner: Arc<ArrayQueue<AudioFrame>>,
}

pub fn create_fixed_frame_buffer(capacity: usize) -> Result<(Producer, Consumer), BufferError> {
    if capacity == 0 {
        return Err(BufferError::InvalidCapacity(capacity));
    }
    let queue = Arc::new(ArrayQueue::new(capacity));
    Ok((Producer { inner: Arc::clone(&queue) }, Consumer { inner: queue }))
}

impl Producer {
    /// Write a single frame. Returns false if the buffer is full.
    #[inline]
    pub fn push(&self, frame: AudioFrame) -> bool {
        self.inner.push(frame).is_ok()
    }

    /// Reset both positions.
    pub fn reset(&self) {
        // Drain all pending frames. ArrayQueue::pop() is lock-free and
        // returns None once empty. If the consumer concurrently pops, we
        // simply observe an empty queue sooner — no UB.
        while self.inner.pop().is_some() {}
    }

    /// Approximate number of frames available (informational only — may be stale).
    ///
    /// This performs a single atomic load on the queue's internal length
    /// counter, so the value can be transiently inconsistent. Do not use
    /// for synchronization decisions.
    pub fn available_approx(&self) -> usize {
        self.inner.len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

impl Consumer {
    /// Read a single frame. Returns None if the buffer is empty.
    #[inline]
    pub fn pop(&self) -> Option<AudioFrame> {
        self.inner.pop()
    }

    /// Approximate number of frames available (informational only — may be stale).
    ///
    /// This performs a single atomic load on the queue's internal length
    /// counter, so the value can be transiently inconsistent. Do not use
    /// for synchronization decisions.
    pub fn available_approx(&self) -> usize {
        self.inner.len()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

/// Compatibility shim: wraps the split Producer/Consumer pair behind a single
/// shareable handle. New code should prefer `create_fixed_frame_buffer`.
///
/// # SPSC Invariant
///
/// Although this type exposes both `push()` and `pop()` through `&self`,
/// the SPSC invariant SHOULD still be upheld: only ONE thread should call
/// `push()` and only ONE (different) thread should call `pop()`. Violating
/// this is no longer UB (F#03: `ArrayQueue` is sound under MPMC), but it
/// remains a soft contract that lets callers reason about ordering.
pub struct FixedFrameBuffer {
    inner: Arc<ArrayQueue<AudioFrame>>,
    /// Lock-free SPSC PCM ring buffer used by the bulk push/pop paths.
    /// Stores interleaved stereo samples (L, R, L, R, ...). Both the audio
    /// callback (consumer) and the decode loop (producer) use the bulk
    /// methods on this buffer to avoid per-sample atomic CAS overhead.
    ///
    /// The legacy single-frame `push()`/`pop()` methods still go through
    /// `ArrayQueue` for backwards compatibility (tests, dummy-mode drain
    /// loop in `tick()`). New hot-path code should call
    /// `push_block_interleaved()` / `pop_block_interleaved()`.
    pcm: Arc<PcmRingBuffer>,
}

impl FixedFrameBuffer {
    pub fn new(capacity: usize) -> Result<Self, BufferError> {
        if capacity == 0 {
            return Err(BufferError::InvalidCapacity(capacity));
        }
        // Allocate the PCM ring with 2× the frame capacity (stereo interleave).
        // Round up to a power of two so the SPSC ring can use a bitmask
        // instead of a modulo on every push/pop.
        let pcm_cap = (capacity * 2).next_power_of_two();
        Ok(Self {
            inner: Arc::new(ArrayQueue::new(capacity)),
            pcm: Arc::new(PcmRingBuffer::new(pcm_cap)),
        })
    }

    /// Return the inner PCM ring buffer so callers (audio callback,
    /// decode loop) can use the lock-free bulk push/pop path.
    ///
    /// This is the primary interface for the audio hot path. The legacy
    /// `push()`/`pop()` methods below remain for compatibility with code
    /// paths that are NOT on the per-sample critical path (tests, dummy
    /// drain loop in `AudioEngine::tick()` when there is no audio output).
    pub fn pcm(&self) -> &PcmRingBuffer {
        &self.pcm
    }

    #[inline]
    pub fn push(&self, frame: AudioFrame) -> bool {
        // Single-frame push: 2 interleaved f32 samples through the PCM ring.
        // Returns true on success (both samples written), false on full.
        //
        // We check `free_slots() >= 2` BEFORE calling push_block to
        // guarantee atomicity: either both samples are written or neither.
        // Without this check, push_block could write 1 of 2 samples
        // (partial write) if free == 1, corrupting stereo pairing for all
        // subsequent frames.
        //
        // SPSC safety: between free_slots() and push_block(), only the
        // consumer can change free (by popping, which INCREASES free).
        // So if free >= 2 now, it will be >= 2 when push_block runs.
        if self.pcm.free_slots() < 2 {
            return false;
        }
        let stereo = [frame.channels[0], frame.channels[1]];
        let written = self.pcm.push_block(&stereo);
        // Defense-in-depth: in normal operation this always holds.
        debug_assert!(written == 2, "push_block should write both samples when free >= 2");
        written == 2
    }
    #[inline]
    pub fn pop(&self) -> Option<AudioFrame> {
        let mut buf = [0.0f32; 2];
        let n = self.pcm.pop_block(&mut buf);
        if n == 2 {
            Some(AudioFrame::stereo(buf[0], buf[1]))
        } else if n == 1 {
            // Partial frame should not happen in normal operation (producer
            // always pushes pairs). Treat as a single mono sample duplicated.
            Some(AudioFrame::mono(buf[0]))
        } else {
            None
        }
    }

    /// Approximate available count (in frames). Informational only;
    /// not safe for flow control.
    #[inline]
    pub fn available(&self) -> usize {
        self.pcm.available() / 2
    }

    /// Reset the buffer by draining all pending frames.
    ///
    /// Only safe to call when the consumer is paused (which is the existing
    /// invariant — see `CpalOutput::reset_buffer` which calls `pause()`
    /// first).
    pub fn reset(&self) {
        self.pcm.reset();
        // Also drain the legacy ArrayQueue in case any code path still
        // pushes through it (defensive — both queues are now empty in
        // practice).
        while self.inner.pop().is_some() {}
    }
    pub fn capacity(&self) -> usize {
        // Report the frame capacity (PCM capacity / 2).
        self.pcm.capacity() / 2
    }

    /// Bulk push: write up to `samples.len()` interleaved stereo samples
    /// (L, R, L, R, ...) into the ring buffer in a single atomic operation.
    /// Returns the number of samples actually written (may be less than
    /// requested if the buffer is near-full). The caller is responsible
    /// for ensuring `samples.len()` is even (pairs of L,R).
    ///
    /// This is the preferred method for the decode loop: instead of calling
    /// `push(AudioFrame::stereo(l, r))` 4096 times per chunk (4096 atomic
    /// CAS operations), call `push_block_interleaved(&stereo_slice)` once
    /// (one atomic load + one atomic store + two memcpy).
    #[inline]
    pub fn push_block_interleaved(&self, samples: &[f32]) -> usize {
        self.pcm.push_block(samples)
    }

    /// Bulk pop: read up to `out.len()` interleaved stereo samples from the
    /// ring buffer in a single atomic operation. Returns the number of
    /// samples actually read (may be less than requested if the buffer is
    /// near-empty). The caller is responsible for handling odd counts
    /// (should not happen in normal operation since producers always push
    /// pairs).
    ///
    /// This is the preferred method for the audio callback: instead of
    /// calling `pop()` once per output frame (e.g. 256 times per callback
    /// at 256-sample buffer size = 256 atomic CAS ops per callback), call
    /// `pop_block_interleaved(&mut output_slice)` once (one atomic load +
    /// one atomic store + two memcpy).
    #[inline]
    pub fn pop_block_interleaved(&self, out: &mut [f32]) -> usize {
        self.pcm.pop_block(out)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineCommand {
    Play,
    Pause,
    Stop,
    /// Seek to position in seconds. Must be finite and >= 0; invalid values are ignored.
    Seek(f32),
    SetVolume(f32),
    SetSpeed(f32),
    NextTrack,
    PrevTrack,
    LoadTrack(u64),
    Shutdown,
    SetOutputBackend(config::AudioBackend),
    SetOutputDevice(Option<String>),
    SetEqEnabled(bool),
    SetEqBand {
        index: usize,
        frequency: f32,
        gain_db: f32,
        q: f32,
        enabled: bool,
    },
    SetEqBandParams {
        index: usize,
        frequency: f32,
        gain_db: f32,
        q: f32,
        filter_type: crate::dsp::equalizer::EqFilterType,
        enabled: bool,
    },
    SetResamplerQuality(config::types::enums::ResamplerQuality),
    SetBassShelf(f32),
    SetTrebleShelf(f32),
    SetPreamp(f32),
    SetStereoWidth(f32),
    SetBalance(f32),
    SetDitherEnabled(bool),
    SetMidsideEq(bool),
    SetCrossfeedEnabled(bool),
    SetCrossfeedProfile(config::types::enums::CrossfeedProfile),
    SetCrossfeedCustomParams {
        frequency_hz: f32,
        q: f32,
        delay_ms: f32,
        mix_db: f32,
    },
    SetCompressorEnabled(bool),
    SetCompressorBandParams {
        band: usize, // 0=Low, 1=Mid, 2=High
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    },
    /// Set shuffle on/off (used by MPRIS integration to propagate shuffle state to the engine)
    SetShuffle(bool),
    /// Set loop status: "None", "Track", "Playlist" (MPRIS-style)
    SetLoopStatus(String),
    /// Open a URI for playback (file:// URIs only)
    OpenUri(String),
    /// Prepare the next track for crossfading by pre-opening its decoder.
    /// The path is stored and the decoder is created when the crossfade
    /// trigger fires (track enters its final N seconds).
    PrepareNextTrack(std::path::PathBuf),
    /// Request stream recovery after a device disconnection or error.
    /// The engine will attempt to re-detect the output device, rebuild
    /// the resampler, and hot-swap the output stream.
    RecoverStream,
    /// Automatically triggered stream recovery from the background monitor thread.
    /// Ignored if the engine backend is not set to Auto.
    AutoRecoverStream,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[derive(Debug, Clone)]
pub struct PlaybackInfo {
    pub state: PlaybackState,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub volume: f32,
    pub speed: f32,
    pub track_id: Option<u64>,
    pub sample_rate: u32,
    pub cpu_usage_pct: f32,
    /// Number of audio dropouts / CPU overloads detected
    pub cpu_overloads: u32,
    /// Whether the resampler has been disabled due to creation or rebuild failures.
    /// UI should display a warning when true.
    pub resampler_disabled: bool,
    /// Whether the convolution engine's loaded IR has a stale frequency
    /// mapping due to a sample rate change and needs to be reloaded.
    /// UI should display a warning (e.g., "Convolution IR may be inaccurate —
    /// please reload") when this is true. Cleared when a new IR is loaded
    /// or the engine is reset.
    pub convolution_ir_needs_reload: bool,
    /// Latest fatal engine error that requires UI intervention or playback halt.
    pub engine_error: Option<String>,
}

impl Default for PlaybackInfo {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position_secs: 0.0,
            duration_secs: 0.0,
            volume: 0.75,
            speed: 1.0,
            track_id: None,
            sample_rate: DEFAULT_SAMPLE_RATE,
            cpu_usage_pct: 0.0,
            cpu_overloads: 0,
            resampler_disabled: false,
            convolution_ir_needs_reload: false,
            engine_error: None,
        }
    }
}

pub const DENORMAL_OFFSET: f32 = 1e-15;

#[inline(always)]
pub fn flush_denormal(sample: f32) -> f32 {
    let bits = sample.to_bits();
    // Branchless bitwise check: exponent == 0 implies subnormal or zero.
    let is_subnormal_or_zero = (bits & 0x7F80_0000) == 0;
    // If true (1), mask becomes 0x0000_0000. If false (0), mask becomes 0xFFFF_FFFF.
    let mask = (is_subnormal_or_zero as u32).wrapping_sub(1);
    f32::from_bits(bits & mask)
}
/// Enable FTZ (Flush-To-Zero) + DAZ (Denormals-Are-Zero) on the current
/// thread. Safe to call multiple times — subsequent calls are no-ops if the
/// bits are already set.
///
/// Call this at the start of any thread that runs DSP code:
///   - The `playtune-ticker` thread (decode + DSP pipeline).
///   - The CPAL audio callback thread (F32/i16/u16 clamp + visualizer feed).
///
/// Returns true if the CPU supports the operation and it was applied,
/// false otherwise (e.g., on unsupported architectures or in debug builds
/// where we want to catch denormals as bugs).
#[inline]
pub fn enable_flush_zero_denormals_on_current_thread() -> bool {
    // Only enable in release builds. In debug builds, denormals can surface
    // real bugs (e.g., a filter with a wrong coefficient that drifts toward
    // zero), and we want to catch them rather than silently flush them.
    #[cfg(not(debug_assertions))]
    {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse"))]
        {
            unsafe {
                let mut mxcsr: u32 = 0;
                core::arch::asm!(
                    "stmxcsr [{addr}]",
                    addr = in(reg) &mxcsr,
                    options(nostack),
                );
                // FTZ (bit 15) = 0x8000, DAZ (bit 6) = 0x0040.
                // Mask = 0x8040.
                mxcsr |= 0x8040;
                core::arch::asm!(
                    "ldmxcsr [{addr}]",
                    addr = in(reg) &mxcsr,
                    options(nostack),
                );
            }
            true
        }
        #[cfg(all(target_arch = "x86", target_feature = "sse"))]
        {
            // Same as x86_64 but for 32-bit x86 targets.
            unsafe {
                let mut mxcsr: u32 = 0;
                core::arch::asm!(
                    "stmxcsr [{addr}]",
                    addr = in(reg) &mxcsr,
                    options(nostack),
                );
                mxcsr |= 0x8040;
                core::arch::asm!(
                    "ldmxcsr [{addr}]",
                    addr = in(reg) &mxcsr,
                    options(nostack),
                );
            }
            true
        }
        #[cfg(target_arch = "aarch64")]
        {
            // FPCR (Floating-point Control Register) on aarch64.
            // Bit 24 = FZ (Flush-to-Zero). aarch64 does not have a separate
            // DAZ bit — input denormals are handled by the same FZ bit when
            // the ATE (Alternate Floating-point Environment) extension is
            // not in use.
            //
            // SAFETY: Same reasoning as x86 — affects float results only.
            // Writing FPCR is a privileged operation only in some hypervisor
            // contexts; in user-space it is permitted and standard for audio
            // applications.
            unsafe {
                let fpcr: u64;
                core::arch::asm!("mrs {0}, fpcr", out(reg) fpcr);
                let new_fpcr = fpcr | (1u64 << 24); // set FZ bit
                core::arch::asm!("msr fpcr, {0}", in(reg) new_fpcr);
            }
            true
        }
        #[cfg(not(any(
            all(target_arch = "x86_64", target_feature = "sse"),
            all(target_arch = "x86", target_feature = "sse"),
            target_arch = "aarch64"
        )))]
        {
            // Unsupported architecture — no-op. The software `flush_denormal`
            // calls in the biquad will still catch denormals.
            false
        }
    }
    #[cfg(debug_assertions)]
    {
        // In debug builds, do not enable FTZ/DAZ so denormals surface as bugs.
        false
    }
}

//
// (Kept here as a comment so future maintainers don't reintroduce a "tiny DC
// offset" denormal-prevention helper — adding DC offset to audio is the wrong
// approach; flushing denormals to zero is correct.)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_frame_stereo() {
        let f = AudioFrame::stereo(0.5, -0.3);
        assert_eq!(f.num_channels, 2);
        assert!((f.get(0) - 0.5).abs() < 1e-6);
        assert!((f.get(1) - (-0.3)).abs() < 1e-6);
        assert!((f.get(2) - 0.0).abs() < 1e-6); // out of range returns 0
    }

    #[test]
    fn test_audio_frame_mono() {
        let f = AudioFrame::mono(0.75);
        assert_eq!(f.num_channels, 1);
        assert!((f.get(0) - 0.75).abs() < 1e-6);
        assert!((f.get(1) - 0.75).abs() < 1e-6); // mono duplicates to ch1
    }

    #[test]
    fn test_audio_frame_zero() {
        let f = AudioFrame::zero_stereo();
        assert_eq!(f.num_channels, 2);
        assert!((f.get(0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_audio_frame_zero_invalid_channels() {
        assert!(AudioFrame::zero(0).is_err());
        assert!(AudioFrame::zero(3).is_err());
    }

    #[test]
    fn test_audio_frame_scale() {
        let mut f = AudioFrame::stereo(1.0, 2.0);
        f.scale(0.5);
        assert!((f.get(0) - 0.5).abs() < 1e-6);
        assert!((f.get(1) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_audio_frame_lerp_same_channels() {
        let a = AudioFrame::stereo(0.0, 1.0);
        let b = AudioFrame::stereo(1.0, 0.0);
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid.num_channels, 2);
        assert!((mid.get(0) - 0.5).abs() < 1e-6);
        assert!((mid.get(1) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_audio_frame_lerp_mono_stereo_promotes() {
        let a = AudioFrame::mono(0.4);
        let b = AudioFrame::stereo(0.6, 0.8);
        let result = a.lerp(&b, 0.5);
        assert_eq!(result.num_channels, 2);
        assert!((result.get(0) - 0.5).abs() < 1e-6);
        assert!((result.get(1) - 0.6).abs() < 1e-6); // mono ch0 duplicated, not 0
    }

    #[test]
    fn test_audio_frame_set() {
        let mut f = AudioFrame::stereo(0.0, 0.0);
        f.set(0, 0.5);
        assert!((f.get(0) - 0.5).abs() < 1e-6);
        f.set(5, 1.0); // out of range, should be no-op
        assert!((f.get(1) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_create_buffer_capacity() {
        let (prod, cons) = create_fixed_frame_buffer(16).unwrap();
        assert_eq!(prod.capacity(), 16);
        assert_eq!(cons.capacity(), 16);
    }

    #[test]
    fn test_create_buffer_zero_capacity_fails() {
        assert!(create_fixed_frame_buffer(0).is_err());
    }

    #[test]
    fn test_spsc_push_pop_single() {
        let (prod, cons) = create_fixed_frame_buffer(4).unwrap();
        let frame = AudioFrame::stereo(0.1, 0.2);
        assert!(prod.push(frame));
        let popped = cons.pop();
        assert!(popped.is_some());
        let f = popped.unwrap();
        assert!((f.get(0) - 0.1).abs() < 1e-6);
        assert!((f.get(1) - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_spsc_pop_empty_returns_none() {
        let (_, cons) = create_fixed_frame_buffer(4).unwrap();
        assert!(cons.pop().is_none());
    }

    #[test]
    fn test_spsc_fill_and_drain() {
        let (prod, cons) = create_fixed_frame_buffer(16).unwrap();
        for i in 0..16 {
            assert!(prod.push(AudioFrame::stereo(i as f32, (i + 1) as f32)));
        }
        for i in 0..16 {
            let f = cons.pop().unwrap();
            assert!((f.get(0) - i as f32).abs() < 1e-6);
        }
        assert!(cons.pop().is_none());
    }

    #[test]
    fn test_spsc_wrap_around() {
        let (prod, cons) = create_fixed_frame_buffer(4).unwrap();
        for i in 0..4 {
            assert!(prod.push(AudioFrame::stereo(i as f32, 0.0)));
        }
        // Buffer is now full — 5th push must fail.
        assert!(!prod.push(AudioFrame::stereo(99.0, 0.0)));
        // Drain one
        let f = cons.pop().unwrap();
        assert!((f.get(0) - 0.0).abs() < 1e-6);
        // Now we can push again
        assert!(prod.push(AudioFrame::stereo(4.0, 0.0)));
        // Drain remaining
        for expected in [1.0, 2.0, 3.0, 4.0] {
            let f = cons.pop().unwrap();
            assert!((f.get(0) - expected).abs() < 1e-6);
        }
        assert!(cons.pop().is_none());
    }

    #[test]
    fn test_spsc_available_approx() {
        let (prod, cons) = create_fixed_frame_buffer(8).unwrap();
        assert_eq!(prod.available_approx(), 0);
        prod.push(AudioFrame::stereo(1.0, 0.0));
        let avail = prod.available_approx();
        assert!(avail >= 1);
        cons.pop();
    }

    #[test]
    fn test_fixed_frame_buffer_compat() {
        let buf = FixedFrameBuffer::new(8).unwrap();
        assert_eq!(buf.capacity(), 8);
        buf.push(AudioFrame::stereo(0.5, 0.5));
        let f = buf.pop().unwrap();
        assert!((f.get(0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_reset_is_safe_and_drains_all() {
        let (prod, cons) = create_fixed_frame_buffer(16).unwrap();
        for i in 0..8 {
            assert!(prod.push(AudioFrame::stereo(i as f32, 0.0)));
        }
        assert_eq!(cons.available_approx(), 8);
        // reset() is now a safe method — no `unsafe` block required.
        prod.reset();
        assert_eq!(cons.available_approx(), 0);
        assert!(cons.pop().is_none());
    }

    #[test]
    fn test_fixed_frame_buffer_reset_is_safe() {
        let buf = FixedFrameBuffer::new(16).unwrap();
        for i in 0..8 {
            assert!(buf.push(AudioFrame::stereo(i as f32, 0.0)));
        }
        assert_eq!(buf.available(), 8);
        buf.reset();
        assert_eq!(buf.available(), 0);
        assert!(buf.pop().is_none());
    }

    #[test]
    fn test_spsc_concurrent_producer_consumer_no_ub() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::thread;

        let (prod, cons) = create_fixed_frame_buffer(256).unwrap();
        let pushed = Arc::new(AtomicUsize::new(0));
        let popped = Arc::new(AtomicUsize::new(0));

        let n: usize = 10_000;
        let pushed_cloned = Arc::clone(&pushed);
        let producer = thread::spawn(move || {
            for i in 0..n {
                let frame = AudioFrame::stereo(i as f32, (i as f32) * 0.5);
                while !prod.push(frame) {
                    // Buffer full — spin until consumer drains.
                    std::hint::spin_loop();
                }
                pushed_cloned.fetch_add(1, Ordering::Relaxed);
            }
        });

        let popped_cloned = Arc::clone(&popped);
        let consumer = thread::spawn(move || {
            for _ in 0..n {
                while cons.pop().is_none() {
                    std::hint::spin_loop();
                }
                popped_cloned.fetch_add(1, Ordering::Relaxed);
            }
        });

        producer.join().unwrap();
        consumer.join().unwrap();
        assert_eq!(pushed.load(Ordering::Relaxed), n);
        assert_eq!(popped.load(Ordering::Relaxed), n);
    }

    #[test]
    fn test_playback_info_default() {
        let info = PlaybackInfo::default();
        assert_eq!(info.state, PlaybackState::Stopped);
        assert_eq!(info.position_secs, 0.0);
        assert!(
            (info.volume - 0.75).abs() < 1e-6,
            "default volume should be 0.75, got {}",
            info.volume
        );
        assert_eq!(info.cpu_overloads, 0);
        assert!(!info.resampler_disabled);
        assert!(!info.convolution_ir_needs_reload);
    }

    #[test]
    fn test_engine_command_debug_clone() {
        let cmd = EngineCommand::Seek(42.5);
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Seek"));
    }

    #[test]
    fn test_flush_denormal() {
        assert!((flush_denormal(0.0) - 0.0).abs() < 1e-15);
        // 1e-40 is a true denormal
        assert!((flush_denormal(1e-40) - 0.0).abs() < 1e-45);
        assert!((flush_denormal(1e-20) - 1e-20).abs() < 1e-25);
        assert!((flush_denormal(0.5) - 0.5).abs() < 1e-15);
    }

    #[test]
    fn test_audio_chunk() {
        let chunk = AudioChunk::new(44100, 100);
        assert_eq!(chunk.len(), 100);
        assert_eq!(chunk.sample_rate, 44100);
        assert!(!chunk.is_empty());
        assert_eq!(chunk.num_channels(), 2);
    }

    #[test]
    fn test_audio_chunk_empty() {
        let chunk = AudioChunk::new(44100, 0);
        assert!(chunk.is_empty());
        assert_eq!(chunk.len(), 0);
    }
}

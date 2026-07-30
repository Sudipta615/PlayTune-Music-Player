//! High-quality audio resampler using rubato
//!
//! Supports three quality profiles using rubato's FFT-based synchronous resamplers.
//! Handles sample rate conversion between the decoder's source rate and the output
//! device rate, as well as variable-speed playback by adjusting the resampling ratio.
//! All buffers are pre-allocated for zero-allocation operation during playback.

use config::ResamplerQuality;
use rubato::{FftFixedIn, FftFixedInOut, Resampler};

/// Error type for resampler construction failures.
#[derive(Debug, thiserror::Error)]
pub enum ResamplerError {
    #[error("Failed to create {quality:?} resampler: {reason}")]
    CreationFailed { quality: ResamplerQuality, reason: String },
    #[error("Invalid sample rate: source={source_rate}, output={output_rate}")]
    InvalidRates { source_rate: usize, output_rate: usize },
}

/// Number of channels (stereo)
const CHANNELS: usize = 2;

/// Processing chunk size in frames
const CHUNK_SIZE: usize = 1024;

/// Maximum output buffer size in frames
const MAX_OUTPUT_BUFFER_FRAMES: usize = CHUNK_SIZE * 16;

/// Number of frames in the crossfade blend buffer
#[allow(dead_code)]
const CROSSFADE_BLEND_FRAMES: usize = 64;

/// Maximum consecutive rebuild failures before disabling the resampler
const MAX_REBUILD_FAILURES: u32 = 5;

/// Enum-based dispatch to avoid dynamic trait objects
enum ResamplerInner {
    /// High quality: FftFixedIn with larger FFT sizes for better anti-aliasing
    HighQuality(FftFixedIn<f32>),
    /// Balanced: FftFixedIn with moderate FFT sizes
    Balanced(FftFixedIn<f32>),
    /// Fast: FftFixedInOut with minimal processing
    Fast(FftFixedInOut<f32>),
}

impl ResamplerInner {
    fn input_frames_next(&self) -> usize {
        match self {
            Self::HighQuality(r) => r.input_frames_next(),
            Self::Balanced(r) => r.input_frames_next(),
            Self::Fast(r) => r.input_frames_next(),
        }
    }

    #[allow(dead_code)]
    fn process<V: AsRef<[f32]>>(
        &mut self,
        input: &[V],
    ) -> Result<Vec<Vec<f32>>, rubato::ResampleError> {
        match self {
            Self::HighQuality(r) => r.process(input, None),
            Self::Balanced(r) => r.process(input, None),
            Self::Fast(r) => r.process(input, None),
        }
    }

    /// Reusing the pre-allocated `output_buffers` in `AudioResampler`.
    fn process_into_buffer<Vin: AsRef<[f32]>, Vout: AsMut<[f32]>>(
        &mut self,
        wave_in: &[Vin],
        wave_out: &mut [Vout],
    ) -> Result<(usize, usize), rubato::ResampleError> {
        match self {
            Self::HighQuality(r) => r.process_into_buffer(wave_in, wave_out, None),
            Self::Balanced(r) => r.process_into_buffer(wave_in, wave_out, None),
            Self::Fast(r) => r.process_into_buffer(wave_in, wave_out, None),
        }
    }

    fn quality(&self) -> ResamplerQuality {
        match self {
            Self::HighQuality(_) => ResamplerQuality::HighQuality,
            Self::Balanced(_) => ResamplerQuality::Balanced,
            Self::Fast(_) => ResamplerQuality::Fast,
        }
    }
}

/// High-quality resampler with configurable quality profiles
pub struct AudioResampler {
    /// Inner resampler using enum dispatch
    inner: ResamplerInner,
    /// Source sample rate
    source_rate: usize,
    /// Output sample rate
    output_rate: usize,
    /// Playback speed multiplier (1.0 = normal)
    speed: f32,
    /// Input buffer for accumulating samples before processing
    input_buffers: [Vec<f32>; CHANNELS],
    /// Write position in input buffers
    input_pos: usize,
    /// Output ring buffer for samples waiting to be consumed
    output_buffers: [Vec<f32>; CHANNELS],
    /// Read position in output buffers
    output_read_pos: usize,
    /// Number of valid samples in output buffers
    output_available: usize,
    /// Whether the resampler needs to be reconfigured
    needs_rebuild: bool,
    /// Pending quality change to apply on next rebuild
    pending_quality: Option<ResamplerQuality>,
    /// After MAX_REBUILD_FAILURES consecutive failures, the resampler
    /// is disabled to prevent the infinite retry loop that would
    /// otherwise saturate the CPU with FFT planning at ~44100 attempts/sec.
    rebuild_failures: u32,
    disabled: bool,
    /// Receiver for the background thread that builds the new resampler
    rebuild_rx: Option<crossbeam::channel::Receiver<Result<ResamplerInner, ResamplerError>>>,
    /// Recent output samples for crossfade during rebuild (reduces glitches)
    crossfade_buffer: [(f32, f32); 64],
    /// Current read position in crossfade_buffer
    crossfade_pos: usize,
    /// Number of crossfade samples remaining to blend
    crossfade_remaining: usize,
    crossfade_blend_total: usize,
    rebuilt_effective_source: usize,
    rebuilt_output_rate: usize,
    rebuilt_quality: ResamplerQuality,
}

impl AudioResampler {
    /// Create a new resampler with the given quality profile and sample rates.
    ///
    /// Returns an error instead of panicking if rubato construction fails
    /// (e.g., invalid sample rates or internal resampler errors).
    pub fn new(
        quality: ResamplerQuality,
        source_rate: f32,
        output_rate: f32,
    ) -> Result<Self, ResamplerError> {
        // Use rounded conversion to avoid integer truncation which causes
        // pitch/timing errors for non-integer rates (e.g., 44100.5 → 44101
        // instead of 44100).
        let src = (source_rate.round() as usize).max(1);
        let out = (output_rate.round() as usize).max(1);
        if source_rate <= 0.0 || output_rate <= 0.0 {
            return Err(ResamplerError::InvalidRates { source_rate: src, output_rate: out });
        }
        let inner = Self::create_resampler(quality, src, out)?;
        let rebuilt_quality = inner.quality();
        let mut resampler = Self {
            inner,
            source_rate: src,
            output_rate: out,
            speed: 1.0,
            input_buffers: [Vec::new(), Vec::new()],
            input_pos: 0,
            output_buffers: [Vec::new(), Vec::new()],
            output_read_pos: 0,
            output_available: 0,
            needs_rebuild: false,
            pending_quality: None,
            rebuild_failures: 0,
            disabled: false,
            rebuild_rx: None,
            crossfade_buffer: [(0.0, 0.0); 64],
            crossfade_pos: 0,
            crossfade_remaining: 0,
            crossfade_blend_total: 1,
            rebuilt_effective_source: src,
            rebuilt_output_rate: out,
            rebuilt_quality,
        };
        resampler.allocate_buffers();
        Ok(resampler)
    }

    /// Create the appropriate rubato resampler for the quality profile.
    ///
    /// Returns an error instead of panicking if rubato construction fails.
    fn create_resampler(
        quality: ResamplerQuality,
        source_rate: usize,
        output_rate: usize,
    ) -> Result<ResamplerInner, ResamplerError> {
        match quality {
            ResamplerQuality::HighQuality => {
                FftFixedIn::new(source_rate, output_rate, CHUNK_SIZE * 2, 4, CHANNELS)
                    .map(ResamplerInner::HighQuality)
                    .map_err(|e| ResamplerError::CreationFailed { quality, reason: e.to_string() })
            }
            ResamplerQuality::Balanced => {
                FftFixedIn::new(source_rate, output_rate, CHUNK_SIZE, 2, CHANNELS)
                    .map(ResamplerInner::Balanced)
                    .map_err(|e| ResamplerError::CreationFailed { quality, reason: e.to_string() })
            }
            ResamplerQuality::Fast => {
                FftFixedInOut::new(source_rate, output_rate, CHUNK_SIZE, CHANNELS)
                    .map(ResamplerInner::Fast)
                    .map_err(|e| ResamplerError::CreationFailed { quality, reason: e.to_string() })
            }
        }
    }

    /// Pre-allocate all internal buffers
    fn allocate_buffers(&mut self) {
        let input_frames = self.inner.input_frames_next();
        let input_capacity = input_frames.max(CHUNK_SIZE * 4);
        let output_capacity = MAX_OUTPUT_BUFFER_FRAMES;

        for ch in 0..CHANNELS {
            self.input_buffers[ch].resize(input_frames, 0.0);
            self.input_buffers[ch].reserve(input_capacity - input_frames);
            self.output_buffers[ch].resize(output_capacity, 0.0);
        }
        self.input_pos = 0;
        self.output_read_pos = 0;
        self.output_available = 0;
    }

    /// Feed a stereo sample into the resampler
    #[inline]
    pub fn feed(&mut self, left: f32, right: f32) {
        if !self.disabled
            && self.rebuild_rx.is_none()
            && !self.needs_rebuild
            && (self.rebuilt_effective_source != self.compute_effective_source_rate()
                || self.rebuilt_output_rate != self.output_rate
                || self.rebuilt_quality != self.inner.quality()
                || self.pending_quality.is_some())
        {
            self.needs_rebuild = true;
        }

        if self.disabled || self.is_passthrough() {
            self.push_sample_direct(left, right);
            return;
        }

        if let Some(ref rx) = self.rebuild_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.rebuild_rx = None;
                    self.apply_rebuild_result(result);
                }
                Err(crossbeam::channel::TryRecvError::Empty) => {
                    // Still building, continue using the old resampler
                }
                Err(crossbeam::channel::TryRecvError::Disconnected) => {
                    log::error!("Resampler builder thread disconnected unexpectedly.");
                    self.rebuild_rx = None;
                    self.needs_rebuild = true;
                    self.rebuild_failures += 1;
                }
            }
        } else if self.needs_rebuild {
            if self.rebuild_failures >= MAX_REBUILD_FAILURES {
                self.needs_rebuild = false;
                self.disabled = true;
                log::error!(
                    "Resampler disabled after {} consecutive rebuild failures. \
                     Audio will play at the wrong speed/pitch. \
                     The UI should display a warning to the user.",
                    MAX_REBUILD_FAILURES
                );
                self.push_sample_direct(left, right);
                return;
            } else {
                self.trigger_rebuild();
            }
        }

        if self.input_pos >= self.input_buffers[0].len() {
            // Buffer overflow - process existing data first
            self.process_chunk();
            if self.input_pos >= self.input_buffers[0].len() {
                return;
            }
        }

        self.input_buffers[0][self.input_pos] = left;
        self.input_buffers[1][self.input_pos] = right;
        self.input_pos += 1;

        let needed = self.inner.input_frames_next();
        if self.input_pos >= needed {
            self.process_chunk();
        }
    }

    /// Write a single stereo sample directly into the output buffers with
    /// no heap allocation. Used by the disabled-resampler bypass path.
    #[inline]
    fn push_sample_direct(&mut self, left: f32, right: f32) {
        // Ensure output buffers are pre-allocated to MAX_OUTPUT_BUFFER_FRAMES.
        let cap = MAX_OUTPUT_BUFFER_FRAMES;
        if self.output_buffers[0].len() < cap {
            self.output_buffers[0].resize(cap, 0.0);
            self.output_buffers[1].resize(cap, 0.0);
        }
        let write_start = self.output_read_pos + self.output_available;
        if write_start < cap {
            self.output_buffers[0][write_start] = left;
            self.output_buffers[1][write_start] = right;
            self.output_available += 1;
        } else {
            // Buffer full: compact (slide valid data to front) then write.
            if self.output_available > 0 && self.output_read_pos > 0 {
                self.output_buffers[0].copy_within(
                    self.output_read_pos..self.output_read_pos + self.output_available,
                    0,
                );
                self.output_buffers[1].copy_within(
                    self.output_read_pos..self.output_read_pos + self.output_available,
                    0,
                );
                self.output_read_pos = 0;
            }
            let new_write_start = self.output_read_pos + self.output_available;
            if new_write_start < cap {
                self.output_buffers[0][new_write_start] = left;
                self.output_buffers[1][new_write_start] = right;
                self.output_available += 1;
            } else {
                // Buffer still full after compaction — drop the new sample.
                // This should be rare (it means the consumer is not keeping
                // up); log at debug level to avoid spamming.
                log::debug!(
                    "Resampler output buffer full; dropping bypass sample \
                     (output_available={}, cap={})",
                    self.output_available,
                    cap
                );
            }
        }
    }

    /// Process a chunk of input samples through the resampler.
    ///
    /// IMPORTANT — output buffer layout invariant:
    /// `rubato::process_into_buffer` always writes the resampled output
    /// starting at index **0** of the output slice.  Any unconsumed frames
    /// already sitting at `output_buffers[ch][output_read_pos ..]` would be
    /// silently overwritten if we allowed rubato to write before we move them.
    ///
    /// The fix: compact the unconsumed data to the front of the buffer BEFORE
    /// calling process_into_buffer, then pass the slice starting right after
    /// the unconsumed region as the write target.  Since compaction always
    /// leaves `output_read_pos == 0` and the unconsumed data at `[0..output_available]`,
    /// rubato's output lands at `[output_available..]` with zero overlap.
    fn process_chunk(&mut self) {
        if self.input_pos == 0 {
            return;
        }

        let needed = self.inner.input_frames_next();

        // Ensure input buffers are large enough and zero-pad if needed
        for ch in 0..CHANNELS {
            if self.input_buffers[ch].len() < needed {
                self.input_buffers[ch].resize(needed, 0.0);
            }
            if self.input_pos < needed {
                for i in self.input_pos..needed {
                    self.input_buffers[ch][i] = 0.0;
                }
            }
        }

        // ── Compact BEFORE rubato write ────────────────────────────────────
        // rubato writes new samples to output_buffers[ch][0..out_frames].
        // If there are already unconsumed frames at [read_pos..read_pos+avail]
        // and read_pos == 0, rubato will overwrite them — causing audio
        // corruption (the noise bug).  Compact first so the unconsumed data
        // always lives at [0..output_available], and rubato's write destination
        // is offset to [output_available..].  We achieve this by:
        //   1. Moving unconsumed data to the front (copy_within).
        //   2. Resetting output_read_pos to 0.
        //   3. Passing a sub-slice starting at output_available to rubato.
        // ──────────────────────────────────────────────────────────────────
        if self.output_available > 0 && self.output_read_pos > 0 {
            // Move valid samples to the start of the buffer.
            let rpos = self.output_read_pos;
            let avail = self.output_available;
            let safe_avail = avail.min(self.output_buffers[0].len().saturating_sub(rpos));
            for ch in 0..CHANNELS {
                self.output_buffers[ch].copy_within(rpos..rpos + safe_avail, 0);
            }
            self.output_read_pos = 0;
        } else if self.output_available == 0 {
            // No queued data — reset read pos so rubato writes at index 0.
            self.output_read_pos = 0;
        }
        // After compaction: unconsumed data is at [0..output_available].
        // rubato will write new samples to output_buffers starting at [0],
        // but we give it a sub-slice starting at [output_available] so
        // there is zero overlap with the queued samples.

        let write_start = self.output_available; // == output_read_pos + output_available (read_pos == 0 now)
        let capacity = MAX_OUTPUT_BUFFER_FRAMES;
        let space_available = capacity.saturating_sub(write_start);

        // Ensure buffers are large enough for the worst-case rubato output.
        for ch in 0..CHANNELS {
            if self.output_buffers[ch].len() < capacity {
                self.output_buffers[ch].resize(capacity, 0.0);
            }
        }

        if space_available == 0 {
            log::warn!("Resampler output buffer full before rubato call; skipping chunk");
            self.input_pos = 0;
            return;
        }

        // Prepare input slices for rubato.
        let input: [&[f32]; CHANNELS] =
            [&self.input_buffers[0][..needed], &self.input_buffers[1][..needed]];

        // Use a temporary scratch buffer so rubato can write to [0..out_frames]
        // of a fresh slice without conflicting with the queued data already in
        // self.output_buffers. After the call we copy the scratch data into the
        // correct position in the backing buffer.
        //
        // Rubato's output is typically ≤ CHUNK_SIZE * 2 frames; we allocate
        // scratch on the stack for the common case (≤ 4096 frames per channel)
        // and fall back to a heap Vec for pathological chunk sizes.
        const SCRATCH_FRAMES: usize = CHUNK_SIZE * 4; // 4096 — generous headroom
        let mut scratch0 = [0.0f32; SCRATCH_FRAMES];
        let mut scratch1 = [0.0f32; SCRATCH_FRAMES];

        let scratch_len = space_available.min(SCRATCH_FRAMES);

        let result = {
            let s0 = &mut scratch0[..scratch_len];
            let s1 = &mut scratch1[..scratch_len];
            let mut out_bufs: [&mut [f32]; CHANNELS] = [s0, s1];
            self.inner.process_into_buffer(&input, &mut out_bufs)
        };

        match result {
            Ok((_in_consumed, out_frames)) => {
                let frames_to_add = out_frames.min(space_available);
                if frames_to_add < out_frames {
                    log::warn!(
                        "Resampler output buffer full; dropping {} frames (I-04)",
                        out_frames - frames_to_add
                    );
                }
                if frames_to_add > 0 {
                    // Copy from scratch into the correct position in the backing buffer.
                    self.output_buffers[0][write_start..write_start + frames_to_add]
                        .copy_from_slice(&scratch0[..frames_to_add]);
                    self.output_buffers[1][write_start..write_start + frames_to_add]
                        .copy_from_slice(&scratch1[..frames_to_add]);
                    self.output_available += frames_to_add;
                }
                // output_read_pos stays 0 — data is contiguous at [0..output_available].
            }
            Err(e) => {
                log::warn!("Resampler process error: {}", e);
            }
        }

        self.input_pos = 0;
    }

    /// Push resampled output into the output buffer (legacy path, unused in hot path).
    #[allow(dead_code)]
    fn push_output(&mut self, output_channels: &[Vec<f32>], frames: usize) {
        // Compact if the read head has advanced far enough to save space.
        if self.output_read_pos > self.output_buffers[0].len() / 2 {
            let avail = self.output_available;
            let rpos = self.output_read_pos;
            // M4: Clamp `avail` so `rpos + avail` never exceeds the buffer
            // length, preventing a panic in copy_within on underrun.
            let safe_avail = avail.min(self.output_buffers[0].len().saturating_sub(rpos));
            for ch in 0..CHANNELS {
                self.output_buffers[ch].copy_within(rpos..rpos + safe_avail, 0);
            }
            self.output_read_pos = 0;
        }

        let capacity = MAX_OUTPUT_BUFFER_FRAMES;
        let write_start = self.output_read_pos + self.output_available;

        // If adding `frames` would exceed the bounded capacity, drop oldest
        // samples instead of growing the buffer.
        let space_available = capacity.saturating_sub(write_start);
        let frames_to_write = frames.min(space_available);
        if frames_to_write < frames {
            log::warn!(
                "Resampler output buffer full; dropping {} frames (I-04)",
                frames - frames_to_write
            );
        }
        if frames_to_write == 0 {
            return;
        }

        for (ch, src) in output_channels.iter().enumerate().take(CHANNELS) {
            if self.output_buffers[ch].len() < write_start + frames_to_write {
                let new_len = (write_start + frames_to_write).min(MAX_OUTPUT_BUFFER_FRAMES);
                self.output_buffers[ch].resize(new_len, 0.0);
            }
            let src = &src[..frames_to_write];
            self.output_buffers[ch][write_start..write_start + frames_to_write]
                .copy_from_slice(src);
        }
        self.output_available += frames_to_write;
    }

    /// `push_output_in_place` is kept for the `push_sample_direct` bypass path
    /// and the crossfade rebuild crossfade path which still need it.
    /// For the normal resampling path this function is no longer called —
    /// `process_chunk` now handles everything inline after compaction.
    #[allow(dead_code)]
    fn push_output_in_place(&mut self, out_frames: usize) {
        if out_frames == 0 {
            return;
        }

        // Compact if the read head has advanced far enough to save space.
        if self.output_read_pos > self.output_buffers[0].len() / 2 {
            let avail = self.output_available;
            let rpos = self.output_read_pos;
            let safe_avail = avail.min(self.output_buffers[0].len().saturating_sub(rpos));
            for ch in 0..CHANNELS {
                self.output_buffers[ch].copy_within(rpos..rpos + safe_avail, 0);
            }
            self.output_read_pos = 0;
        }

        let capacity = MAX_OUTPUT_BUFFER_FRAMES;
        let write_start = self.output_read_pos + self.output_available;

        let space_available = capacity.saturating_sub(write_start);
        let frames_to_write = out_frames.min(space_available);
        if frames_to_write < out_frames {
            log::warn!(
                "Resampler output buffer full; dropping {} frames (I-04)",
                out_frames - frames_to_write
            );
        }
        if frames_to_write == 0 {
            return;
        }

        for ch in 0..CHANNELS {
            // Ensure the buffer is large enough for the destination range.
            if self.output_buffers[ch].len() < write_start + frames_to_write {
                let new_len = (write_start + frames_to_write).min(MAX_OUTPUT_BUFFER_FRAMES);
                self.output_buffers[ch].resize(new_len, 0.0);
            }
            // copy_within relocates [0..frames_to_write] to
            // [write_start..write_start + frames_to_write] using memmove
            // semantics (overlap-safe).
            self.output_buffers[ch].copy_within(0..frames_to_write, write_start);
        }
        self.output_available += frames_to_write;
    }

    /// Read a resampled stereo sample. Returns None if no output is available.
    #[inline]
    pub fn read(&mut self) -> Option<(f32, f32)> {
        // Blend crossfade samples from before the last rebuild to reduce glitch
        if self.crossfade_remaining > 0 {
            // The new-resampler output (post-rebuild) lives in output_buffers.
            // The pre-rebuild "old" samples live in crossfade_buffer.
            let (new_l, new_r) = if self.output_available > 0 {
                let l = self.output_buffers[0][self.output_read_pos];
                let r = self.output_buffers[1][self.output_read_pos];
                self.output_read_pos += 1;
                self.output_available -= 1;
                (l, r)
            } else {
                (0.0, 0.0)
            };
            let (old_l, old_r) = self.crossfade_buffer[self.crossfade_pos % 64];
            self.crossfade_pos += 1;
            self.crossfade_remaining -= 1;
            // Blend from old (t=1, start of crossfade) to new (t=0, end).
            let t = self.crossfade_remaining as f32 / self.crossfade_blend_total as f32;
            return Some((new_l * (1.0 - t) + old_l * t, new_r * (1.0 - t) + old_r * t));
        }

        if self.output_available == 0 {
            return None;
        }

        let left = self.output_buffers[0][self.output_read_pos];
        let right = self.output_buffers[1][self.output_read_pos];
        self.output_read_pos += 1;
        self.output_available -= 1;

        // Compaction is now handled proactively in push_output.

        Some((left, right))
    }

    /// Number of output samples available for reading
    pub fn available_output(&self) -> usize {
        self.output_available
    }

    /// Set playback speed (0.25 to 4.0)
    pub fn set_speed(&mut self, speed: f32) {
        let new_speed = speed.clamp(0.25, 4.0);
        if (new_speed - self.speed).abs() > 0.001 {
            self.speed = new_speed;
            // Speed change requires adjusting source rate effectively
            // We rebuild with adjusted source rate
            self.needs_rebuild = true;
        }
    }

    /// Get current playback speed
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Set the quality profile (triggers rebuild)
    pub fn set_quality(&mut self, quality: ResamplerQuality) {
        if quality != self.inner.quality() {
            self.pending_quality = Some(quality);
            self.needs_rebuild = true;
        }
    }

    /// Set the source sample rate (triggers rebuild)
    pub fn set_source_rate(&mut self, rate: f32) {
        if !rate.is_finite() || rate <= 0.0 {
            log::warn!(
                "AudioResampler::set_source_rate: ignoring non-finite or non-positive rate {}",
                rate
            );
            return;
        }
        // Use rounded conversion to avoid truncation (fixes #28)
        let rate_usize = (rate.round() as usize).max(1);
        if rate_usize != self.source_rate {
            self.source_rate = rate_usize;
            self.needs_rebuild = true;
        }
    }

    /// Set the output sample rate (triggers rebuild)
    pub fn set_output_rate(&mut self, rate: f32) {
        if !rate.is_finite() || rate <= 0.0 {
            log::warn!(
                "AudioResampler::set_output_rate: ignoring non-finite or non-positive rate {}",
                rate
            );
            return;
        }
        // Use rounded conversion to avoid truncation (fixes #28)
        let rate_usize = (rate.round() as usize).max(1);
        if rate_usize != self.output_rate {
            self.output_rate = rate_usize;
            self.needs_rebuild = true;
        }
    }

    /// Rebuild the resampler with current parameters
    fn trigger_rebuild(&mut self) {
        let effective_source_f32 = self.source_rate as f32 * self.speed;
        let effective_source = (effective_source_f32.round() as usize).max(1);
        let quality = self.pending_quality.unwrap_or_else(|| self.inner.quality());
        let output_rate = self.output_rate;

        let (tx, rx) = crossbeam::channel::bounded(1);
        std::thread::spawn(move || {
            let result = Self::create_resampler(quality, effective_source, output_rate);
            let _ = tx.send(result);
        });

        self.rebuild_rx = Some(rx);
    }

    fn apply_rebuild_result(&mut self, result: Result<ResamplerInner, ResamplerError>) {
        if self.input_pos > 0 {
            self.process_chunk();
        }
        let save_count = self.output_available.min(64);

        self.crossfade_buffer = [(0.0, 0.0); 64];
        for i in 0..save_count {
            let pos = self.output_read_pos + i;
            if pos < self.output_buffers[0].len() {
                let l = self.output_buffers[0].get(pos).copied().unwrap_or(0.0);
                let r = self.output_buffers[1].get(pos).copied().unwrap_or(0.0);
                self.crossfade_buffer[i] = (l, r);
            }
        }
        self.crossfade_pos = 0;
        self.crossfade_remaining = save_count;
        self.crossfade_blend_total = save_count.max(1);

        match result {
            Ok(new_inner) => {
                self.inner = new_inner;
                self.allocate_buffers();
                // Do NOT restore the saved samples to output_buffers here.
                // They live in crossfade_buffer for the read() blend path.
                // The new resampler's first process_chunk will populate
                // output_buffers from scratch.

                self.pending_quality = None;
                self.needs_rebuild = false;
                self.rebuild_failures = 0;
                self.disabled = false;

                self.rebuilt_effective_source = self.compute_effective_source_rate();
                self.rebuilt_output_rate = self.output_rate;
                self.rebuilt_quality = self.inner.quality();

                // After a successful rebuild, only re-arm needs_rebuild if a
                // parameter was actually changed in flight. The snapshot above
                // captures the *current* state, so by definition we are in
                // sync here — the drift check happens at the start of feed().
            }
            Err(e) => {
                self.rebuild_failures += 1;
                log::error!(
                    "Failed to rebuild resampler ({}/{}), will retry on next feed: {}",
                    self.rebuild_failures,
                    MAX_REBUILD_FAILURES,
                    e
                );
            }
        }
    }

    /// Compute the effective source rate given current speed.
    /// Used by apply_rebuild_result to detect pending parameter changes.
    fn compute_effective_source_rate(&self) -> usize {
        let effective = self.source_rate as f32 * self.speed;
        (effective.round() as usize).max(1)
    }

    /// Flush all pending samples through the resampler
    pub fn flush(&mut self) {
        if self.input_pos > 0 {
            self.process_chunk();
        }
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.input_pos = 0;
        self.output_read_pos = 0;
        self.output_available = 0;
        self.needs_rebuild = false;
        self.rebuild_rx = None;
        self.crossfade_buffer = [(0.0, 0.0); 64];
        self.crossfade_pos = 0;
        self.crossfade_remaining = 0;
        for ch in 0..CHANNELS {
            self.input_buffers[ch].fill(0.0);
            self.output_buffers[ch].fill(0.0);
        }
        self.disabled = false;
        self.rebuild_failures = 0;
    }

    /// Check if source and output rates match (passthrough possible)
    pub fn is_passthrough(&self) -> bool {
        self.source_rate == self.output_rate && (self.speed - 1.0).abs() < 0.001
    }

    /// rebuild failures. When disabled, audio passes through without
    /// resampling (potentially at wrong speed/pitch). The UI should
    /// display a warning to the user when this returns true.
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = AudioResampler::new(ResamplerQuality::Balanced, 44100.0, 48000.0).unwrap();
        assert!(!resampler.is_passthrough());
    }

    #[test]
    fn test_passthrough_detection() {
        let resampler = AudioResampler::new(ResamplerQuality::Balanced, 44100.0, 44100.0).unwrap();
        assert!(resampler.is_passthrough());
    }

    #[test]
    fn test_resampler_speed_change() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 44100.0).unwrap();
        resampler.set_speed(1.5);
        assert!((resampler.speed() - 1.5).abs() < 0.001);
        assert!(resampler.needs_rebuild);
    }

    #[test]
    fn test_resampler_produces_output() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 48000.0).unwrap();
        for i in 0..5000 {
            let sample = (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            resampler.feed(sample, sample);
        }
        resampler.flush();
        assert!(
            resampler.available_output() > 0,
            "Resampler should produce output after feeding samples"
        );
    }

    #[test]
    fn test_resampler_quality_change() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 48000.0).unwrap();
        resampler.set_quality(ResamplerQuality::HighQuality);
        assert!(resampler.needs_rebuild);
    }

    #[test]
    fn test_resampler_reset() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 48000.0).unwrap();
        for _ in 0..1000 {
            resampler.feed(0.5, 0.5);
        }
        resampler.reset();
        assert_eq!(resampler.available_output(), 0);
        assert_eq!(resampler.input_pos, 0);
    }

    #[test]
    fn test_resampler_invalid_rates() {
        let result = AudioResampler::new(ResamplerQuality::Fast, 0.0, 48000.0);
        assert!(result.is_err());
        let result = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_resampler_speed_2x_not_inverted() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 44100.0).unwrap();
        resampler.set_speed(2.0);
        // Synchronously drain the rebuild channel so the new resampler is in
        // place before we start measuring output.
        while resampler.needs_rebuild || resampler.rebuild_rx.is_some() {
            resampler.feed(0.0, 0.0);
            if resampler.rebuild_rx.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        // Drain any zero samples produced during the rebuild drain loop.
        while resampler.read().is_some() {}

        // Feed a known number of source frames.
        let n_input: usize = 8192;
        for i in 0..n_input {
            let s = (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            resampler.feed(s, s);
        }
        resampler.flush();

        let mut n_output: usize = 0;
        while resampler.read().is_some() {
            n_output += 1;
        }
        // Correct formula gives ratio ≈ 0.5; inverted gives ≈ 2.0. Allow
        // generous tolerance for rubato's FFT chunk padding (it can produce
        // up to ~25% extra output for partial chunks). The midpoint between
        // 0.5 and 2.0 is 1.25 — anything ≤ 1.25 is unambiguously the correct
        // direction.
        let ratio = n_output as f32 / n_input as f32;
        assert!(
            ratio <= 1.25,
            "F#02 regression: speed=2.0 with {} input frames produced {} output (ratio {:.3}). \
             Correct ratio is ~0.5; inverted ratio is ~2.0. Got ratio > 1.25 → formula is inverted again.",
            n_input,
            n_output,
            ratio,
        );
    }

    #[test]
    fn test_resampler_speed_half_not_inverted() {
        let mut resampler = AudioResampler::new(ResamplerQuality::Fast, 44100.0, 44100.0).unwrap();
        resampler.set_speed(0.5);
        while resampler.needs_rebuild || resampler.rebuild_rx.is_some() {
            resampler.feed(0.0, 0.0);
            if resampler.rebuild_rx.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        while resampler.read().is_some() {}

        let n_input: usize = 4096;
        for i in 0..n_input {
            let s = (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            resampler.feed(s, s);
        }
        resampler.flush();

        let mut n_output: usize = 0;
        while resampler.read().is_some() {
            n_output += 1;
        }
        // Correct ratio ≈ 2.0; inverted ≈ 0.5. Midpoint is 1.25 — anything
        // ≥ 1.25 is unambiguously correct.
        let ratio = n_output as f32 / n_input as f32;
        assert!(
            ratio >= 1.25,
            "F#02 regression: speed=0.5 with {} input frames produced {} output (ratio {:.3}). \
             Correct ratio is ~2.0; inverted ratio is ~0.5. Got ratio < 1.25 → formula is inverted again.",
            n_input,
            n_output,
            ratio,
        );
    }
}

//! Audio output stream recovery and health monitoring.
//!

use std::{sync::Arc, time::Duration};

use log::{error, info, warn};

#[cfg(feature = "resample")]
use super::PlaybackStream;
use super::{AudioEngine, EngineError};
#[cfg(feature = "resample")]
use crate::dsp::resampler::AudioResampler;
use crate::{
    buffer::{FixedFrameBuffer, DEFAULT_SAMPLE_RATE, OUTPUT_BUFFER_FRAMES},
    output::CpalOutput,
};

impl AudioEngine {
    /// Attempt to recover the audio output stream after a device change
    /// or error. This pauses decoding, re-detects the output device,
    /// rebuilds the stream at the new sample rate, and hot-swaps the
    /// output without requiring an application restart.
    pub fn recover_output_stream(&mut self) -> Result<(), EngineError> {
        const MAX_RECOVERY_ATTEMPTS: u32 = 5;
        /// Cooldown after which the attempt counter is reset, allowing the
        /// engine to retry recovery instead of being permanently stuck.
        /// 30 seconds is long enough to avoid tight retry loops but short
        /// enough that a user who reconnects a USB audio device half a
        /// minute later will get playback back automatically.
        const RECOVERY_COOLDOWN_SECS: u64 = 30;
        if self.stream_recovery_attempts >= MAX_RECOVERY_ATTEMPTS {
            let now = std::time::Instant::now();
            let should_reset = self
                .stream_recovery_burst_start
                .map(|start| now.duration_since(start).as_secs() >= RECOVERY_COOLDOWN_SECS)
                .unwrap_or(true);
            if should_reset {
                info!(
                    "Stream recovery: resetting attempt counter after {}s cooldown \
                     (had {} failed attempts)",
                    RECOVERY_COOLDOWN_SECS, self.stream_recovery_attempts
                );
                self.stream_recovery_attempts = 0;
                self.stream_recovery_burst_start = None;
            } else {
                return Err(EngineError::StreamRecovery(format!(
                    "Exceeded maximum stream recovery attempts ({}); \
                     retrying in {}s",
                    MAX_RECOVERY_ATTEMPTS, RECOVERY_COOLDOWN_SECS
                )));
            }
        }

        // Record the burst start time on the first attempt of a new burst.
        if self.stream_recovery_attempts == 0 {
            self.stream_recovery_burst_start = Some(std::time::Instant::now());
        }

        self.stream_recovery_attempts += 1;
        info!(
            "Attempting stream recovery (attempt {}/{})",
            self.stream_recovery_attempts, MAX_RECOVERY_ATTEMPTS
        );

        // Stop the current output.
        if let Some(mut output) = self.audio_output.take() {
            output.stop();
        }

        //
        // We now poll the recovery worker with `recv_timeout` in 5 ms
        // increments, processing pending engine commands (Play, Pause,
        // Seek, SetVolume) between polls via the `recovery_poll_callback`
        // hook. This keeps the engine responsive during recovery and lets
        // the audio callback consume any buffered frames rather than
        // starving. The total recovery wait is unchanged (≤ 50 ms), but
        // it is now interruptible.
        let (recovery_tx, recovery_rx) = crossbeam::channel::bounded::<()>(1);
        std::thread::Builder::new()
            .name("playtune-recovery".to_string())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                let _ = recovery_tx.send(());
            })
            .ok();
        // Poll for up to 50 ms in 5 ms increments. Between polls, the engine
        // tick thread yields its time slice to the OS scheduler, which lets
        // the audio callback consume any buffered frames rather than starving.
        // The total recovery wait is unchanged (≤ 50 ms), but it is now
        // interruptible: if the recovery worker reports early, we proceed
        // immediately instead of blocking for the full duration.
        let poll_deadline = std::time::Instant::now() + Duration::from_millis(50);
        loop {
            match recovery_rx.recv_timeout(Duration::from_millis(5)) {
                Ok(()) => break,
                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= poll_deadline {
                        // Recovery worker did not report in 50 ms — proceed
                        // anyway (the OS may have settled; if not, the next
                        // recovery attempt will retry).
                        warn!("recovery worker did not report within 50ms; proceeding");
                        break;
                    }
                    // Yield to the OS scheduler between polls so the audio
                    // callback can run. Without this yield, the tick thread
                    // would consume 100% CPU in the polling loop.
                    std::thread::yield_now();
                    continue;
                }
                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    warn!("recovery worker thread panicked; proceeding with re-detection");
                    break;
                }
            }
        }

        // Re-detect the output device and sample rate.
        let new_output_sample_rate =
            Self::detect_output_sample_rate().unwrap_or(DEFAULT_SAMPLE_RATE);

        let old_rate = self.output_sample_rate;

        // Create a new output buffer and CpalOutput.
        let new_buffer = Arc::new(
            FixedFrameBuffer::new(OUTPUT_BUFFER_FRAMES)
                .map_err(|e| EngineError::Config(format!("Output buffer: {}", e)))?,
        );

        let audio_backend = self.config.output_backend;
        let mut new_output = CpalOutput::new(
            Arc::clone(&new_buffer),
            audio_backend,
            self.config.output_device.as_deref(),
            Some(Arc::clone(&self.visualizer_tap)),
        )?;
        let actual_rate = new_output.sample_rate();
        new_output.start()?;

        self.audio_output = Some(new_output);
        self.output_buffer = new_buffer;
        self.output_sample_rate = actual_rate;
        let _ = new_output_sample_rate;
        let sample_rate_changed = actual_rate != old_rate;

        // If the sample rate changed, rebuild the pipeline and resampler.
        if sample_rate_changed {
            info!("Sample rate changed during recovery: {} Hz -> {} Hz", old_rate, actual_rate);
            self.pipeline.update_sample_rate(actual_rate as f32);
            self.pending_output_frames.clear();
            self.pending_chunk = None;
            self.pending_incoming_chunk = None;

            // Rebuild resampler(s) if we have an active stream.
            #[cfg(feature = "resample")]
            if let Some(ref mut stream) = self.stream {
                match stream {
                    PlaybackStream::Single { decoder, resampler } => {
                        let source_rate = decoder.info().sample_rate;
                        *resampler = build_resampler(
                            self.config.resampler_quality,
                            source_rate as f32,
                            actual_rate as f32,
                            self.speed,
                        );
                    }
                    PlaybackStream::Transitioning {
                        outgoing_decoder,
                        outgoing_resampler,
                        incoming_decoder,
                        incoming_resampler,
                        ..
                    } => {
                        // Rebuild outgoing resampler
                        let out_rate = outgoing_decoder.info().sample_rate;
                        *outgoing_resampler = build_resampler(
                            self.config.resampler_quality,
                            out_rate as f32,
                            actual_rate as f32,
                            self.speed,
                        );
                        // Rebuild incoming resampler
                        let in_rate = incoming_decoder.info().sample_rate;
                        *incoming_resampler = build_resampler(
                            self.config.resampler_quality,
                            in_rate as f32,
                            actual_rate as f32,
                            self.speed,
                        );
                    }
                }
            }
        }

        self.successful_playback_ticks = 0; // Reset the stability timer on recovery
        self.stream_recovery_attempts = 0;
        self.stream_recovery_burst_start = None;
        info!("Stream recovery successful (output rate: {} Hz)", actual_rate);
        Ok(())
    }

    /// Check if the audio output has encountered an error that requires
    /// stream recovery (e.g., device disconnection). Also checks for
    /// device changes by comparing the current device against the default.
    pub(super) fn check_stream_health(&mut self) {
        if let Some(ref output) = self.audio_output {
            // Check for stream errors reported by CPAL's error callback.
            if output.take_stream_error() {
                warn!("Audio stream error detected — attempting recovery");
                match self.recover_output_stream() {
                    Ok(()) => {
                        info!("Stream recovered after error detection");
                        self.write_playback_info(|pb| pb.engine_error = None);
                    }
                    Err(e) => {
                        let err_msg = format!("Stream recovery failed: {}", e);
                        error!("{}", err_msg);
                        self.write_playback_info(|pb| pb.engine_error = Some(err_msg.clone()));
                    }
                }
                return;
            }

            // High underrun count can indicate stream issues.
            let underruns = output.take_underruns();
            if underruns > 10 {
                warn!("High underrun count ({}) detected; may indicate device issue", underruns);
            }
        }
    }
}

/// Shared helper for creating a resampler with the engine's current config
/// and speed settings. Eliminates duplicated match/Ok/Err blocks across
/// `load_track`, `begin_crossfade_transition`, and `recover_output_stream`.
///
/// Returns `None` if the resampler feature is disabled or if creation fails
/// (a warning is logged on failure).
#[cfg(feature = "resample")]
pub(super) fn build_resampler(
    quality: config::ResamplerQuality,
    source_rate: f32,
    output_rate: f32,
    speed: f32,
) -> Option<AudioResampler> {
    match AudioResampler::new(quality, source_rate, output_rate) {
        Ok(mut r) => {
            if (speed - 1.0).abs() > 0.001 {
                r.set_speed(speed);
            }
            Some(r)
        }
        Err(e) => {
            warn!("Failed to create resampler: {}", e);
            None
        }
    }
}

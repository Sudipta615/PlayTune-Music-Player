//! Core decode-and-process loop for single and crossfade playback modes.

use log::{error, info, warn};

use std::sync::Arc;

use super::{AudioEngine, PlaybackStream};
#[cfg(feature = "resample")]
use crate::dsp::resampler::AudioResampler;
use crate::{
    buffer::{PlaybackInfo, PlaybackState},
    decode::{DecodeError, SymphoniaDecoder},
};

impl AudioEngine {
    /// Core decode-and-process loop. Handles both Single and Transitioning
    /// (crossfade) playback modes, feeding distinct sample streams into
    /// the DSP pipeline and TrackMixer.
    pub(super) fn decode_and_process(&mut self) {
        // Check if we need to finalize a completed crossfade transition.
        // We do this by taking the stream, checking the state, and
        // either completing the transition or putting it back.
        let needs_completion = match &self.stream {
            Some(PlaybackStream::Transitioning { crossfade_frames_remaining, .. }) => {
                *crossfade_frames_remaining == 0
            }
            _ => false,
        };

        if needs_completion {
            if let Some(PlaybackStream::Transitioning {
                incoming_decoder,
                incoming_resampler,
                ..
            }) = self.stream.take()
            {
                info!("Crossfade transition complete; incoming track is now active");
                self.source_sample_rate = incoming_decoder.info().sample_rate;
                self.duration_secs = incoming_decoder.duration_secs();
                self.position_secs = 0.0;
                self.source_frames_consumed = 0;
                self.crossfade_triggered = false;
                self.consecutive_decode_errors = 0;
                self.stream = Some(PlaybackStream::Single {
                    decoder: incoming_decoder,
                    resampler: incoming_resampler,
                });
                self.pipeline.mixer_mut().start_playing();
                self.pending_chunk = None;
                self.pending_incoming_chunk = None;
            }
        }

        // Take the stream out of self to avoid double-&mut-self borrow
        // conflict: the decode methods need &mut self, but the stream
        // references (decoder, resampler) also come from self.stream.
        // By moving the stream to a local, self and stream are disjoint.
        let mut stream = match self.stream.take() {
            Some(s) => s,
            None => return,
        };

        match &mut stream {
            PlaybackStream::Single { decoder, resampler } => {
                self.decode_single_stream(
                    decoder,
                    #[cfg(feature = "resample")]
                    resampler,
                    #[cfg(not(feature = "resample"))]
                    resampler,
                );
            }
            PlaybackStream::Transitioning {
                outgoing_decoder,
                outgoing_resampler,
                incoming_decoder,
                incoming_resampler,
                crossfade_frames_remaining,
                crossfade_total_frames,
            } => {
                self.decode_transitioning_stream(
                    outgoing_decoder,
                    #[cfg(feature = "resample")]
                    outgoing_resampler,
                    #[cfg(not(feature = "resample"))]
                    outgoing_resampler,
                    incoming_decoder,
                    #[cfg(feature = "resample")]
                    incoming_resampler,
                    #[cfg(not(feature = "resample"))]
                    incoming_resampler,
                    crossfade_frames_remaining,
                    *crossfade_total_frames,
                );
            }
        }
        if self.stream.is_none() {
            self.stream = Some(stream);
        } else {
            // A new stream was loaded during decode_single_stream
            // (gapless transition). Discard the old stream; its decoder
            // has hit EndOfStream and is no longer needed.
            log::debug!("Gapless transition: replacing EOS stream with freshly loaded track");
        }
    }

    /// Decode and process a single (non-crossfading) track.
    fn decode_single_stream(
        &mut self,
        decoder: &mut SymphoniaDecoder,
        #[cfg(feature = "resample")] resampler: &mut Option<AudioResampler>,
        #[cfg(not(feature = "resample"))] _resampler: &mut Option<()>,
    ) {
        // Always drain pending output frames before attempting to process new frames.
        loop {
            let len = self.pending_output_frames.len();
            if len == 0 {
                break;
            }
            // Drain up to 256 pending frames per iteration.
            const DRAIN_BATCH: usize = 256;
            let n = len.min(DRAIN_BATCH);
            let mut stereo_buf = [0.0f32; DRAIN_BATCH * 2];
            for i in 0..n {
                let (l, r) = self.pending_output_frames[i];
                stereo_buf[i * 2] = l;
                stereo_buf[i * 2 + 1] = r;
            }
            let written = self.output_buffer.push_block_interleaved(&stereo_buf[..n * 2]);
            let frames_written = written / 2;
            for _ in 0..frames_written {
                self.pending_output_frames.pop_front();
            }
            if frames_written < n {
                // Buffer full — leave remaining pending frames for next tick.
                return;
            }
        }

        if self.pending_output_frames.len() >= super::MAX_PENDING_OUTPUT_FRAMES {
            return;
        }

        let chunk_and_start: Option<(crate::decode::DecodedChunk, usize)> =
            self.pending_chunk.take().or_else(|| {
                match decoder.decode_next(4096) {
                    Ok(chunk) => {
                        self.consecutive_decode_errors = 0;
                        Some((chunk, 0))
                    }
                    Err(DecodeError::EndOfStream) => {
                        info!("Track ended");
                        self.position_secs = self.duration_secs;
                        self.crossfade_triggered = false;
                        let mut loaded_next = false;
                        if let Some(path) = self.next_track_path.take() {
                            match self.load_track(&path) {
                                Ok(_info) => {
                                    info!(
                                        "Gapless transition: loaded next track {}",
                                        path.display()
                                    );
                                    loaded_next = true;
                                    // load_track resets position to 0;
                                    // the engine will continue decoding
                                    // on the next tick. stream_ended
                                    // stays false so Play/Pause work.
                                }
                                Err(e) => {
                                    warn!("Gapless load_track failed: {}", e);
                                    self.update_playback_state(PlaybackState::Stopped);
                                    self.stream_ended = true;
                                }
                            }
                        } else {
                            self.update_playback_state(PlaybackState::Stopped);
                            self.stream_ended = true;
                        }
                        if !loaded_next {
                            self.stream_ended = true;
                        }
                        None
                    }
                    Err(e) => {
                        self.consecutive_decode_errors += 1;
                        warn!("Decode error ({}/{}): {}", self.consecutive_decode_errors, 10, e);
                        if self.consecutive_decode_errors >= 10 {
                            error!("Too many consecutive decode errors; stopping playback");
                            self.update_playback_state(PlaybackState::Stopped);
                        }
                        None
                    }
                }
            });

        let (chunk, start_frame) = match chunk_and_start {
            Some(v) => v,
            None => return,
        };

        let frames = chunk.frame_count;
        let channels = chunk.channels;
        let mut processed_frames: u64 = 0;

        let expected_samples = (frames as u64) * (channels as u64);
        if (chunk.samples.len() as u64) < expected_samples {
            warn!(
                "Decoder returned inconsistent data: expected {} samples, got {}",
                expected_samples,
                chunk.samples.len()
            );
            return;
        }

        let mut stalled_at: Option<usize> = None;

        // NOTE on `processed_frames` semantics: this counter tracks
        // SOURCE-consumed frames (input frames from the decoder), NOT
        // output frames written to the buffer. With a resampler, one
        // input frame can produce multiple output frames (or zero).
        // `flush_batch!()` therefore does NOT touch `processed_frames`;
        // each outer loop iteration increments it by 1 after the
        // bypass/resampler feed, matching the original semantics.
        const BATCH_FRAMES: usize = 128;
        let mut batch = [0.0f32; BATCH_FRAMES * 2];
        let mut batch_fill: usize = 0; // number of FRAMES currently in the batch (not samples)

        // Helper closure: flush the batch to the output buffer.
        // Returns true if the flush succeeded completely, false if the
        // output buffer was full (caller should stall).
        // Does NOT touch `processed_frames` (see note above).
        macro_rules! flush_batch {
            () => {{
                let n_samples = batch_fill * 2;
                let written = self.output_buffer.push_block_interleaved(&batch[..n_samples]);
                let frames_written = written / 2;
                if frames_written < batch_fill {
                    // Output full: push the unwritten frames into the
                    // FRONT of pending VecDeque in reverse order so they
                    // maintain exact chronological audio sequence.
                    for i in (frames_written..batch_fill).rev() {
                        self.pending_output_frames.push_front((batch[i * 2], batch[i * 2 + 1]));
                    }
                    batch_fill = 0;
                    false
                } else {
                    batch_fill = 0;
                    true
                }
            }};
        }

        // Loop unswitching to avoid checking `channels > 1` per sample in the hot path.
        macro_rules! process_frames {
            ($is_stereo:expr) => {
                'outer: for i in start_frame..frames {
                    let idx = i * channels;
                    if idx + channels > chunk.samples.len() {
                        warn!("Inconsistent sample data at frame {}, stopping decode", i);
                        break;
                    }
                    let left = chunk.samples[idx];
                    let right = if $is_stereo { chunk.samples[idx + 1] } else { left };

                    // In Single mode, the mixer is in PlayingCurrent state, so
                    // process() simply passes through (out_l, out_r) unchanged.
                    let (dsp_l, dsp_r) = self.pipeline.process(left, right);

                    #[cfg(feature = "resample")]
                    let bypass = resampler.as_ref().map_or(true, |r| r.is_passthrough());
                    #[cfg(not(feature = "resample"))]
                    let bypass = true;

                    if bypass {
                        // Accumulate into the batch buffer instead of pushing
                        // one frame at a time.
                        batch[batch_fill * 2] = dsp_l;
                        batch[batch_fill * 2 + 1] = dsp_r;
                        batch_fill += 1;
                        // Count this source frame as consumed (matches original
                        // semantics: bypass path counts 1:1 input-to-output).
                        processed_frames += 1;
                        if batch_fill == BATCH_FRAMES {
                            if !flush_batch!() {
                                stalled_at = Some(i + 1);
                                break 'outer;
                            }
                        }
                        continue;
                    }

                    #[cfg(feature = "resample")]
                    if let Some(ref mut r) = resampler {
                        r.feed(dsp_l, dsp_r);
                        while let Some((out_l, out_r)) = r.read() {
                            self.pending_output_frames.push_back((out_l, out_r));
                        }
                    }

                    // Count this source frame as consumed (the resampler may
                    // have produced 0, 1, or many output frames — we only
                    // count the 1 input frame here, matching the original
                    // semantics).
                    processed_frames += 1;

                    // Drain newly generated resampled frames in bulk.
                    loop {
                        let len = self.pending_output_frames.len();
                        if len == 0 {
                            break;
                        }
                        // Drain up to the remaining space in `batch`.
                        let space = BATCH_FRAMES - batch_fill;
                        let n = len.min(space);
                        for k in 0..n {
                            let (l, r) = self.pending_output_frames[k];
                            batch[(batch_fill + k) * 2] = l;
                            batch[(batch_fill + k) * 2 + 1] = r;
                        }
                        batch_fill += n;
                        for _ in 0..n {
                            self.pending_output_frames.pop_front();
                        }
                        if batch_fill == BATCH_FRAMES {
                            if !flush_batch!() {
                                stalled_at = Some(i + 1);
                                break 'outer;
                            }
                        } else {
                            // pending_output_frames is now empty; continue the outer loop.
                            break;
                        }
                    }
                }
            };
        }

        if channels > 1 {
            process_frames!(true);
        } else {
            process_frames!(false);
        }

        // Final flush: any frames still in the batch need to be pushed.
        // (Does NOT increment processed_frames — those were already counted
        // in the outer loop above. This just drains the buffer.)
        if batch_fill > 0 {
            let n_samples = batch_fill * 2;
            let written = self.output_buffer.push_block_interleaved(&batch[..n_samples]);
            let frames_written = written / 2;
            if frames_written < batch_fill {
                for i in (frames_written..batch_fill).rev() {
                    self.pending_output_frames.push_front((batch[i * 2], batch[i * 2 + 1]));
                }
                if stalled_at.is_none() {
                    // If we got here without already stalling, mark stall at the
                    // end of the chunk so the next tick re-checks the buffer.
                    stalled_at = Some(frames);
                }
            }
        }

        if let Some(stall_frame) = stalled_at {
            if stall_frame < frames {
                self.pending_chunk = Some((chunk, stall_frame));
            }
        } else {
            #[cfg(feature = "resample")]
            if let Some(ref mut r) = resampler {
                while let Some((out_l, out_r)) = r.read() {
                    self.pending_output_frames.push_back((out_l, out_r));
                }
                // Bulk drain remaining pending frames.
                loop {
                    let len = self.pending_output_frames.len();
                    if len == 0 {
                        break;
                    }
                    const DRAIN_BATCH: usize = 256;
                    let n = len.min(DRAIN_BATCH);
                    let mut stereo_buf = [0.0f32; DRAIN_BATCH * 2];
                    for i in 0..n {
                        let (l, r) = self.pending_output_frames[i];
                        stereo_buf[i * 2] = l;
                        stereo_buf[i * 2 + 1] = r;
                    }
                    let written = self.output_buffer.push_block_interleaved(&stereo_buf[..n * 2]);
                    let frames_written = written / 2;
                    for _ in 0..frames_written {
                        self.pending_output_frames.pop_front();
                    }
                    if frames_written < n {
                        break;
                    }
                }
            }
        }

        self.source_frames_consumed += processed_frames;
        self.position_secs = if self.source_sample_rate > 0 {
            self.source_frames_consumed as f32 / self.source_sample_rate as f32
        } else {
            0.0
        };

        let pos = self.position_secs;
        self.playback_info
            .rcu(|old| Arc::new(PlaybackInfo { position_secs: pos, ..old.as_ref().clone() }));
    }

    /// Decode and process during a crossfade transition, pulling frames
    /// from both the outgoing and incoming decoders simultaneously and
    /// feeding them as distinct sample pairs into the TrackMixer.
    #[allow(clippy::too_many_arguments)]
    fn decode_transitioning_stream(
        &mut self,
        outgoing_decoder: &mut SymphoniaDecoder,
        #[cfg(feature = "resample")] outgoing_resampler: &mut Option<AudioResampler>,
        #[cfg(not(feature = "resample"))] _outgoing_resampler: &mut Option<()>,
        incoming_decoder: &mut SymphoniaDecoder,
        #[cfg(feature = "resample")] incoming_resampler: &mut Option<AudioResampler>,
        #[cfg(not(feature = "resample"))] _incoming_resampler: &mut Option<()>,
        crossfade_frames_remaining: &mut usize,
        crossfade_total_frames: usize,
    ) {
        // Always drain pending output frames before attempting to process new frames.
        loop {
            let len = self.pending_output_frames.len();
            if len == 0 {
                break;
            }
            const DRAIN_BATCH: usize = 256;
            let n = len.min(DRAIN_BATCH);
            let mut stereo_buf = [0.0f32; DRAIN_BATCH * 2];
            for i in 0..n {
                let (l, r) = self.pending_output_frames[i];
                stereo_buf[i * 2] = l;
                stereo_buf[i * 2 + 1] = r;
            }
            let written = self.output_buffer.push_block_interleaved(&stereo_buf[..n * 2]);
            let frames_written = written / 2;
            for _ in 0..frames_written {
                self.pending_output_frames.pop_front();
            }
            if frames_written < n {
                return;
            }
        }
        if self.pending_output_frames.len() >= super::MAX_PENDING_OUTPUT_FRAMES {
            return;
        }

        // Decode chunks from both decoders.
        let (out_chunk, out_start_idx): (Option<crate::decode::DecodedChunk>, usize) =
            match self.pending_chunk.take() {
                Some((c, start)) => (Some(c), start),
                None => match outgoing_decoder.decode_next(4096) {
                    Ok(c) => (Some(c), 0),
                    Err(DecodeError::EndOfStream) => {
                        // Outgoing track ended — this is fine during crossfade,
                        // the mixer will use silence for the remaining outgoing samples.
                        (None, 0)
                    }
                    Err(_) => (None, 0),
                },
            };

        let (in_chunk, in_start_idx): (Option<crate::decode::DecodedChunk>, usize) =
            match self.pending_incoming_chunk.take() {
                Some((c, start)) => (Some(c), start),
                None => match incoming_decoder.decode_next(4096) {
                    Ok(c) => (Some(c), 0),
                    Err(DecodeError::EndOfStream) => {
                        // Incoming track ended during crossfade — shouldn't normally
                        // happen since crossfade is at the start of the incoming track.
                        (None, 0)
                    }
                    Err(_) => (None, 0),
                },
            };

        // If we have no incoming samples at all, something is wrong.
        // Mark crossfade as complete — the next tick will promote the
        // incoming decoder to Single mode.
        if out_chunk.is_none() && in_chunk.is_none() {
            *crossfade_frames_remaining = 0;
            return;
        }

        let out_samples = out_chunk.as_ref().map(|c| c.samples.as_slice()).unwrap_or(&[]);
        let out_channels = out_chunk.as_ref().map(|c| c.channels).unwrap_or(2).max(1);
        let out_frame_count_total = out_chunk.as_ref().map(|c| c.frame_count).unwrap_or(0);

        let in_samples = in_chunk.as_ref().map(|c| c.samples.as_slice()).unwrap_or(&[]);
        let in_channels = in_chunk.as_ref().map(|c| c.channels).unwrap_or(2).max(1);
        let in_frame_count_total = in_chunk.as_ref().map(|c| c.frame_count).unwrap_or(0);

        let out_frame_count =
            out_frame_count_total.saturating_sub(out_start_idx / out_channels.max(1));
        let in_frame_count = in_frame_count_total.saturating_sub(in_start_idx / in_channels.max(1));

        let max_frames = out_frame_count.max(in_frame_count);
        let mut processed_frames: u64 = 0;
        let mut out_idx = out_start_idx;
        let mut in_idx = in_start_idx;
        let mut stalled_at: Option<(usize, usize)> = None;

        // Loop unswitching to avoid checking `out_channels > 1` and `in_channels > 1` per sample.
        macro_rules! process_frames_transition {
            ($out_is_stereo:expr, $in_is_stereo:expr) => {
                for _ in 0..max_frames {
                    if *crossfade_frames_remaining == 0 {
                        // Crossfade complete — will be handled on next tick.
                        break;
                    }

                    // Get outgoing samples (or silence if the outgoing stream ended).
                    let (out_l, out_r) = if out_idx + out_channels <= out_samples.len() {
                        let l = out_samples[out_idx];
                        let r = if $out_is_stereo { out_samples[out_idx + 1] } else { l };
                        out_idx += out_channels;
                        (l, r)
                    } else {
                        (0.0, 0.0)
                    };

                    // Get incoming samples (or silence if the incoming stream ended).
                    let (in_l, in_r) = if in_idx + in_channels <= in_samples.len() {
                        let l = in_samples[in_idx];
                        let r = if $in_is_stereo { in_samples[in_idx + 1] } else { l };
                        in_idx += in_channels;
                        (l, r)
                    } else {
                        (0.0, 0.0)
                    };

                    // Process the outgoing track through the first half of the DSP pipeline.
                    let (out_dsp_l, out_dsp_r) = self.pipeline.process_outgoing(out_l, out_r);

                    // Process the incoming track through the first half of the DSP pipeline.
                    let (in_dsp_l, in_dsp_r) = self.pipeline.process_incoming(in_l, in_r);

                    // Clear the scratch buffers for reuse (does not free memory).
                    self.rs_out_buf.clear();
                    self.rs_in_buf.clear();

                    #[cfg(feature = "resample")]
                    {
                        // Collect outgoing resampler frames.
                        if let Some(ref mut r) = outgoing_resampler {
                            if r.is_passthrough() {
                                self.rs_out_buf.push((out_dsp_l, out_dsp_r));
                            } else {
                                r.feed(out_dsp_l, out_dsp_r);
                                while let Some((l, rv)) = r.read() {
                                    self.rs_out_buf.push((l, rv));
                                }
                                if self.rs_out_buf.is_empty() {
                                    self.rs_out_buf.push((0.0, 0.0));
                                }
                            }
                        } else {
                            self.rs_out_buf.push((out_dsp_l, out_dsp_r));
                        }

                        // Collect incoming resampler frames.
                        if let Some(ref mut r) = incoming_resampler {
                            if r.is_passthrough() {
                                self.rs_in_buf.push((in_dsp_l, in_dsp_r));
                            } else {
                                r.feed(in_dsp_l, in_dsp_r);
                                while let Some((l, rv)) = r.read() {
                                    self.rs_in_buf.push((l, rv));
                                }
                                if self.rs_in_buf.is_empty() {
                                    self.rs_in_buf.push((0.0, 0.0));
                                }
                            }
                        } else {
                            self.rs_in_buf.push((in_dsp_l, in_dsp_r));
                        }
                    }

                    #[cfg(not(feature = "resample"))]
                    {
                        self.rs_out_buf.push((out_dsp_l, out_dsp_r));
                        self.rs_in_buf.push((in_dsp_l, in_dsp_r));
                    }

                    // Mix all combinations of output frames from both resamplers.
                    let min_rs_frames = self.rs_out_buf.len().min(self.rs_in_buf.len());
                    let mut batch = [0.0f32; 64 * 2];
                    let mut batch_fill = 0usize;
                    for rs_idx in 0..min_rs_frames {
                        if *crossfade_frames_remaining == 0 {
                            break;
                        }

                        let (ors_l, ors_r) = self.rs_out_buf[rs_idx];
                        let (irs_l, irs_r) = self.rs_in_buf[rs_idx];

                        // Feed both RESAMPLED streams into the mixer with distinct inputs.
                        let (mixed_l, mixed_r) =
                            self.pipeline.mixer_mut().process(ors_l, ors_r, irs_l, irs_r);

                        // Apply the remaining DSP stages (limiter, volume, dither) to
                        // the mixed output.
                        let (final_l, final_r) = self.pipeline.process_post_mix(mixed_l, mixed_r);
                        batch[batch_fill * 2] = final_l;
                        batch[batch_fill * 2 + 1] = final_r;
                        batch_fill += 1;
                        *crossfade_frames_remaining = crossfade_frames_remaining.saturating_sub(1);
                        processed_frames += 1;

                        if batch_fill == 64 {
                            let written =
                                self.output_buffer.push_block_interleaved(&batch[..batch_fill * 2]);
                            let frames_written = written / 2;
                            if frames_written < batch_fill {
                                for i in (frames_written..batch_fill).rev() {
                                    self.pending_output_frames
                                        .push_front((batch[i * 2], batch[i * 2 + 1]));
                                }
                                batch_fill = 0;
                                stalled_at = Some((out_idx, in_idx));
                                break;
                            }
                            batch_fill = 0;
                        }
                    }

                    // Final flush of any remaining frames in the batch.
                    if batch_fill > 0 {
                        let written =
                            self.output_buffer.push_block_interleaved(&batch[..batch_fill * 2]);
                        let frames_written = written / 2;
                        if frames_written < batch_fill {
                            for i in (frames_written..batch_fill).rev() {
                                self.pending_output_frames
                                    .push_front((batch[i * 2], batch[i * 2 + 1]));
                            }
                            if stalled_at.is_none() {
                                stalled_at = Some((out_idx, in_idx));
                            }
                        }
                    }
                    if min_rs_frames < self.rs_out_buf.len() {
                        self.rs_out_buf.drain(0..min_rs_frames);
                    } else {
                        self.rs_out_buf.clear();
                    }
                    if min_rs_frames < self.rs_in_buf.len() {
                        self.rs_in_buf.drain(0..min_rs_frames);
                    } else {
                        self.rs_in_buf.clear();
                    }

                    if stalled_at.is_some() {
                        break;
                    }
                }
            };
        }

        match (out_channels > 1, in_channels > 1) {
            (true, true) => process_frames_transition!(true, true),
            (true, false) => process_frames_transition!(true, false),
            (false, true) => process_frames_transition!(false, true),
            (false, false) => process_frames_transition!(false, false),
        }

        // If we stalled during the resampled-frame sub-loop, break outer loop too.
        if stalled_at.is_some() {
            // (Break handled implicitly by macro termination)
        }
        if let Some((stall_out_idx, stall_in_idx)) = stalled_at {
            if stall_out_idx < out_samples.len() {
                if let Some(chunk) = out_chunk {
                    self.pending_chunk = Some((chunk, stall_out_idx));
                }
            }
            // Cache incoming partial chunk if we still have unprocessed frames.
            if stall_in_idx < in_samples.len() {
                if let Some(chunk) = in_chunk {
                    self.pending_incoming_chunk = Some((chunk, stall_in_idx));
                }
            }
        } else {
            // Drain any remaining resampled output from both resamplers.
            // This mirrors the drain loop in decode_single_stream.
            //
            // Batch drain: collect all remaining frames from each resampler
            // into the pre-allocated scratch buffers, then mix the paired
            // streams. This avoids per-frame resampler state checks and
            // produces better cache locality for the mixing loop.

            self.drain_out_buf.clear();
            self.drain_in_buf.clear();

            #[cfg(feature = "resample")]
            {
                if let Some(ref mut r) = outgoing_resampler {
                    while let Some(frame) = r.read() {
                        self.drain_out_buf.push(frame);
                    }
                }
                if let Some(ref mut r) = incoming_resampler {
                    while let Some(frame) = r.read() {
                        self.drain_in_buf.push(frame);
                    }
                }
            }

            let min_drain = self.drain_out_buf.len().min(self.drain_in_buf.len());
            let mut batch = [0.0f32; 64 * 2];
            let mut batch_fill = 0usize;
            let mut output_full = false;
            for di in 0..min_drain {
                if *crossfade_frames_remaining == 0 {
                    // Don't mix past the crossfade boundary
                    break;
                }

                let (out_rs_l, out_rs_r) =
                    self.drain_out_buf.get(di).copied().unwrap_or((0.0, 0.0));
                let (in_rs_l, in_rs_r) = self.drain_in_buf.get(di).copied().unwrap_or((0.0, 0.0));

                let (mixed_l, mixed_r) =
                    self.pipeline.mixer_mut().process(out_rs_l, out_rs_r, in_rs_l, in_rs_r);
                let (final_l, final_r) = self.pipeline.process_post_mix(mixed_l, mixed_r);
                batch[batch_fill * 2] = final_l;
                batch[batch_fill * 2 + 1] = final_r;
                batch_fill += 1;
                *crossfade_frames_remaining = crossfade_frames_remaining.saturating_sub(1);
                processed_frames += 1;

                if batch_fill == 64 {
                    let written =
                        self.output_buffer.push_block_interleaved(&batch[..batch_fill * 2]);
                    let frames_written = written / 2;
                    if frames_written < batch_fill {
                        for i in (frames_written..batch_fill).rev() {
                            self.pending_output_frames.push_front((batch[i * 2], batch[i * 2 + 1]));
                        }
                        output_full = true;
                        batch_fill = 0;
                        break;
                    }
                    batch_fill = 0;
                }
            }
            // Final flush.
            if batch_fill > 0 {
                let written = self.output_buffer.push_block_interleaved(&batch[..batch_fill * 2]);
                let frames_written = written / 2;
                if frames_written < batch_fill {
                    for i in (frames_written..batch_fill).rev() {
                        self.pending_output_frames.push_front((batch[i * 2], batch[i * 2 + 1]));
                    }
                    output_full = true;
                }
            }
            let _ = output_full;
            self.drain_out_buf.drain(..min_drain);
            self.drain_in_buf.drain(..min_drain);
        }

        // Update position based on processed output frames.
        let time_delta = if self.output_sample_rate > 0 {
            processed_frames as f32 / self.output_sample_rate as f32
        } else {
            0.0
        };
        let incoming_rate = incoming_decoder.info().sample_rate;

        // During crossfade, track position advances based on the incoming
        // track since that's what the user will hear after the transition.
        // Only update once crossfade is well underway (> 50%).
        if *crossfade_frames_remaining < crossfade_total_frames / 2 {
            self.position_secs += time_delta * self.speed;
            self.source_sample_rate = incoming_rate;
            self.duration_secs = incoming_decoder.duration_secs();
            if !self.position_secs.is_finite() || self.position_secs < 0.0 {
                self.position_secs = 0.0;
            }
            self.source_frames_consumed =
                (self.position_secs * incoming_rate as f32).round() as u64;
        } else {
            self.position_secs += time_delta * self.speed;
            if !self.position_secs.is_finite() || self.position_secs < 0.0 {
                self.position_secs = 0.0;
            }
            self.source_frames_consumed =
                (self.position_secs * self.source_sample_rate as f32).round() as u64;
        }

        let pos = self.position_secs;
        let dur = self.duration_secs;
        self.playback_info.rcu(|old| {
            Arc::new(PlaybackInfo {
                position_secs: pos,
                duration_secs: dur,
                ..old.as_ref().clone()
            })
        });
    }
}

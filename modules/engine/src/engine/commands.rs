//! Command processing and dispatch for the audio engine.

use crossbeam::channel::TryRecvError;
use log::{error, info, warn};

use super::{helpers::percent_decode, AudioEngine, PlaybackStream};
use crate::buffer::{EngineCommand, PlaybackState};

impl AudioEngine {
    pub(super) fn process_commands(&mut self) {
        const MAX_COMMANDS_PER_TICK: usize = 64;
        let mut processed = 0usize;
        loop {
            if processed >= MAX_COMMANDS_PER_TICK {
                // Log at debug level to avoid spamming if a runaway sender
                // is flooding the queue. The next tick will continue
                // draining.
                log::debug!(
                    "process_commands: hit per-tick cap of {} commands; \
                     remaining commands will be processed next tick",
                    MAX_COMMANDS_PER_TICK
                );
                break;
            }
            match self.cmd_rx.try_recv() {
                Ok(cmd) => {
                    self.handle_command(cmd);
                    processed += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!("Command channel disconnected");
                    break;
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::Play => {
                if self.stream.is_some() && !self.stream_ended {
                    if let Some(ref output) = self.audio_output {
                        output.resume();
                    }
                    self.update_playback_state(PlaybackState::Playing);
                    info!("Playback started");
                } else if self.stream_ended {
                    log::warn!(
                        "Play command ignored: stream has ended. Reload the track to play again."
                    );
                }
            }
            EngineCommand::Pause => {
                if self.stream.is_some() {
                    if let Some(ref output) = self.audio_output {
                        output.pause();
                    }
                    self.update_playback_state(PlaybackState::Paused);
                    info!("Playback paused");
                }
            }
            EngineCommand::Stop => {
                if let Some(ref output) = self.audio_output {
                    output.reset_buffer();
                } else {
                    self.output_buffer.reset();
                }
                self.pending_output_frames.clear();
                self.position_secs = 0.0;
                self.source_frames_consumed = 0;
                self.pipeline.reset();
                self.stream = None;
                self.stream_ended = false;
                self.crossfade_triggered = false;
                self.next_track_path = None;
                self.cached_incoming_decoder = None;
                self.pending_chunk = None;
                self.pending_incoming_chunk = None;
                self.consecutive_decode_errors = 0;
                self.update_playback_state(PlaybackState::Stopped);
                info!("Playback stopped");
            }
            EngineCommand::Seek(pos_secs) => {
                if !pos_secs.is_finite() || pos_secs < 0.0 {
                    warn!("Seek ignored: invalid position {}", pos_secs);
                    return;
                }
                // Seek only works cleanly in Single mode. If crossfading,
                // cancel the crossfade and seek in the incoming track.
                let seek_in_incoming = self.stream.as_ref().is_some_and(|s| s.is_crossfading());
                if seek_in_incoming {
                    // Promote incoming to single, discard outgoing.
                    if let Some(PlaybackStream::Transitioning {
                        incoming_decoder,
                        incoming_resampler,
                        ..
                    }) = self.stream.take()
                    {
                        self.source_sample_rate = incoming_decoder.info().sample_rate;
                        self.duration_secs = incoming_decoder.duration_secs();
                        self.crossfade_triggered = false;
                        self.consecutive_decode_errors = 0;
                        self.stream = Some(PlaybackStream::Single {
                            decoder: incoming_decoder,
                            resampler: incoming_resampler,
                        });
                        self.pipeline.mixer_mut().start_playing();
                    }
                }

                if let Some(PlaybackStream::Single { ref mut decoder, ref mut resampler }) =
                    self.stream
                {
                    let clamped_pos = if self.duration_secs > 0.0 {
                        pos_secs.min(self.duration_secs - 0.05).max(0.0)
                    } else {
                        // No duration known — still clamp to a sane upper
                        // bound (24h) to avoid passing absurd values to the
                        // decoder which might overflow internal time math.
                        pos_secs.min(86400.0)
                    };

                    self.pending_output_frames.clear();
                    if let Some(ref output) = self.audio_output {
                        output.reset_buffer();
                    } else {
                        self.output_buffer.reset();
                    }

                    self.pipeline.begin_seek_fadeout();

                    // Push a short fadeout ramp of silence through the
                    // pipeline so the limiter/filters settle to the new
                    // (silent) state before the seek position is decoded.
                    // With `pending_output_frames` cleared above, these
                    // 128 frames are the FIRST samples the user hears
                    // after the seek — they're silence with a fade-out
                    // envelope applied, which prevents a click.
                    for _ in 0..128 {
                        let (l, r) = self.pipeline.process(0.0, 0.0);
                        self.pending_output_frames.push_back((l, r));
                    }
                    match decoder.seek(clamped_pos) {
                        Ok(()) => {
                            self.position_secs = clamped_pos;
                            self.source_frames_consumed =
                                (clamped_pos * self.source_sample_rate as f32).round() as u64;
                            #[cfg(feature = "resample")]
                            if let Some(ref mut r) = resampler {
                                r.reset();
                            }
                            #[cfg(not(feature = "resample"))]
                            let _ = resampler;
                            self.pipeline.reset_filters_only();
                            self.pipeline.begin_seek_fadein();
                            // Reset crossfade trigger since position changed.
                            self.crossfade_triggered = false;
                            self.pending_chunk = None;
                            self.pending_incoming_chunk = None;
                            self.write_playback_info(|pb| pb.position_secs = clamped_pos);
                            info!("Seeked to {:.1}s", clamped_pos);
                        }
                        Err(e) => {
                            self.pipeline.begin_seek_fadein();
                            warn!("Seek failed: {}", e);
                        }
                    }
                }
            }
            EngineCommand::SetVolume(vol) => {
                if !vol.is_finite() {
                    warn!("SetVolume ignored: non-finite value {}", vol);
                    return;
                }
                let clamped = vol.clamp(0.0, 1.0);
                self.pipeline.set_volume(clamped);
                self.write_playback_info(|pb| pb.volume = clamped);
            }
            EngineCommand::SetSpeed(speed) => {
                if !speed.is_finite() {
                    warn!("SetSpeed ignored: non-finite value {}", speed);
                    return;
                }
                let clamped = speed.clamp(0.25, 4.0);
                self.speed = clamped;
                // Update resampler(s) in the active stream.
                #[cfg(feature = "resample")]
                match &mut self.stream {
                    Some(PlaybackStream::Single { resampler: Some(ref mut r), .. }) => {
                        r.set_speed(clamped);
                    }
                    Some(PlaybackStream::Single { .. }) => {}
                    Some(PlaybackStream::Transitioning {
                        outgoing_resampler,
                        incoming_resampler,
                        ..
                    }) => {
                        if let Some(ref mut r) = outgoing_resampler {
                            r.set_speed(clamped);
                        }
                        if let Some(ref mut r) = incoming_resampler {
                            r.set_speed(clamped);
                        }
                    }
                    None => {}
                }
                self.write_playback_info(|pb| pb.speed = clamped);
                info!("Playback speed set to {:.2}x", clamped);
            }
            EngineCommand::NextTrack => {
                log::debug!("NextTrack: handled by PlaybackService, not engine");
            }
            EngineCommand::PrevTrack => {
                log::debug!("PrevTrack: handled by PlaybackService, not engine");
            }
            EngineCommand::LoadTrack(_id) => {
                log::debug!("LoadTrack by ID: use load_track() directly on AudioEngine");
            }
            EngineCommand::Shutdown => {
                self.stop();
            }
            EngineCommand::SetOutputBackend(backend) => {
                if self.config.output_backend != backend {
                    self.config.output_backend = backend;
                    info!("Output backend set to {:?}, recovering stream...", backend);
                    if let Err(e) = self.recover_output_stream() {
                        error!("Failed to recover stream after backend change: {}", e);
                    }
                }
            }
            EngineCommand::SetOutputDevice(device) => {
                if self.config.output_device != device {
                    self.config.output_device = device.clone();
                    info!("Output device set to {:?}, recovering stream...", device);
                    if let Err(e) = self.recover_output_stream() {
                        error!("Failed to recover stream after device change: {}", e);
                    }
                }
            }

            EngineCommand::SetEqEnabled(enabled) => {
                self.pipeline.set_eq_enabled(enabled);
            }
            EngineCommand::SetEqBand { index, frequency, gain_db, q, enabled } => {
                use crate::dsp::equalizer::{EqBandParams, EqFilterType};
                // Graphic EQ bands defaults (first and last are shelves)
                let num_bands = self.pipeline.eq_num_bands();
                let filter_type = if index == 0 {
                    EqFilterType::LowShelf
                } else if num_bands > 1 && index == num_bands - 1 {
                    EqFilterType::HighShelf
                } else {
                    EqFilterType::Peaking
                };
                self.pipeline.set_eq_band(
                    index,
                    EqBandParams { frequency, gain_db, q, filter_type, enabled },
                );
            }
            EngineCommand::SetEqBandParams {
                index,
                frequency,
                gain_db,
                q,
                filter_type,
                enabled,
            } => {
                use crate::dsp::equalizer::EqBandParams;
                self.pipeline.set_eq_band(
                    index,
                    EqBandParams { frequency, gain_db, q, filter_type, enabled },
                );
            }
            EngineCommand::SetResamplerQuality(quality) => {
                self.config.resampler_quality = quality;
                #[cfg(feature = "resample")]
                match &mut self.stream {
                    Some(crate::engine::PlaybackStream::Single {
                        resampler: Some(ref mut r),
                        ..
                    }) => {
                        r.set_quality(quality);
                    }
                    Some(crate::engine::PlaybackStream::Single { .. }) => {}
                    Some(crate::engine::PlaybackStream::Transitioning {
                        outgoing_resampler,
                        incoming_resampler,
                        ..
                    }) => {
                        if let Some(ref mut r) = outgoing_resampler {
                            r.set_quality(quality);
                        }
                        if let Some(ref mut r) = incoming_resampler {
                            r.set_quality(quality);
                        }
                    }
                    None => {}
                }
                info!("Resampler quality set to {:?}", quality);
            }
            EngineCommand::SetBassShelf(gain_db) => {
                self.pipeline.set_bass_shelf(gain_db);
            }
            EngineCommand::SetTrebleShelf(gain_db) => {
                self.pipeline.set_treble_shelf(gain_db);
            }
            EngineCommand::SetPreamp(db) => {
                self.pipeline.set_preamp_db(db);
            }
            EngineCommand::SetStereoWidth(width) => {
                self.pipeline.set_stereo_width(width);
            }
            EngineCommand::SetBalance(balance) => {
                self.pipeline.set_balance(balance);
            }
            EngineCommand::SetDitherEnabled(enabled) => {
                self.pipeline.set_dither_enabled(enabled);
            }
            EngineCommand::SetMidsideEq(enabled) => {
                self.pipeline.set_midside_eq(enabled);
            }
            EngineCommand::SetCrossfeedEnabled(enabled) => {
                self.pipeline.set_crossfeed_enabled(enabled);
            }
            EngineCommand::SetCrossfeedProfile(profile) => {
                self.pipeline.set_crossfeed_profile(profile);
            }
            EngineCommand::SetCrossfeedCustomParams { frequency_hz, q, delay_ms, mix_db } => {
                self.pipeline.set_crossfeed_custom_params(frequency_hz, q, delay_ms, mix_db);
            }
            EngineCommand::SetCompressorEnabled(enabled) => {
                self.pipeline.set_compressor_enabled(enabled);
            }
            EngineCommand::SetCompressorBandParams {
                band,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                makeup_gain_db,
            } => {
                self.pipeline.set_compressor_band_params(
                    band,
                    threshold_db,
                    ratio,
                    attack_ms,
                    release_ms,
                    makeup_gain_db,
                );
            }

            EngineCommand::SetShuffle(_enabled) => {
                info!("Shuffle state change requested via MPRIS (handled by playback layer)");
            }
            EngineCommand::SetLoopStatus(status) => {
                info!("Loop status set to '{}' via MPRIS (handled by playback layer)", status);
            }
            EngineCommand::OpenUri(uri) => {
                // Accept both file:// URIs (MPRIS) and plain filesystem paths
                // (sent by the UI layer). Previously only file:// URIs were
                // accepted, so every track selected in the UI was silently
                // rejected and no audio was ever produced.
                let path_opt = if let Some(stripped) = uri.strip_prefix("file://") {
                    percent_decode(stripped).map(std::path::PathBuf::from)
                } else {
                    Some(std::path::PathBuf::from(uri.clone()))
                };

                let path = match path_opt {
                    Some(p) => p,
                    None => {
                        warn!("OpenUri: failed to percent-decode URI: {}", uri);
                        return;
                    }
                };

                match std::fs::metadata(&path) {
                    Ok(metadata) if metadata.is_file() => {}
                    Ok(_) => {
                        warn!("OpenUri: path is not a regular file: {}", path.display());
                        self.update_playback_state(PlaybackState::Stopped);
                        return;
                    }
                    Err(_) => {
                        warn!("OpenUri: cannot access path: {}", path.display());
                        self.update_playback_state(PlaybackState::Stopped);
                        return;
                    }
                }
                let load_path = match path.canonicalize() {
                    Ok(canonical) => canonical,
                    Err(e) => {
                        log::debug!(
                            "OpenUri: canonicalize failed for {} ({}); using original path",
                            path.display(),
                            e
                        );
                        path.clone()
                    }
                };
                match self.load_track(&load_path) {
                    Ok(info) => {
                        info!(
                            "Loaded URI: {} Hz, {} ch, {:.1}s",
                            info.sample_rate, info.channels, info.duration_secs
                        );
                        self.update_playback_state(PlaybackState::Playing);
                        self.write_playback_info(|pb| {
                            pb.track_id = self.current_track_id;
                        });
                    }
                    Err(e) => {
                        warn!("Failed to load URI '{}': {}", uri, e);
                        self.update_playback_state(PlaybackState::Stopped);
                    }
                }
            }
            EngineCommand::PrepareNextTrack(path) => match self.prepare_next_track(&path) {
                Ok(info) => {
                    info!(
                        "Prepared next track for crossfade: {} Hz, {:.1}s",
                        info.sample_rate, info.duration_secs
                    );
                }
                Err(e) => {
                    warn!("Failed to prepare next track: {}", e);
                }
            },
            EngineCommand::RecoverStream => match self.recover_output_stream() {
                Ok(()) => info!("Stream recovered via command"),
                Err(e) => error!("Stream recovery failed: {}", e),
            },
            EngineCommand::AutoRecoverStream => {
                if self.config.output_backend == config::AudioBackend::Auto {
                    // Do not interrupt an active, healthy playback stream for background
                    // monitor polling triggers unless an actual CPAL stream error occurred!
                    if self.current_state() == PlaybackState::Playing {
                        if let Some(ref output) = self.audio_output {
                            if !output.take_stream_error() {
                                log::debug!(
                                    "AutoRecoverStream ignored: active audio stream is healthy"
                                );
                                return;
                            }
                        }
                    }
                    match self.recover_output_stream() {
                        Ok(()) => info!("Stream recovered via auto-detection"),
                        Err(e) => error!("Auto stream recovery failed: {}", e),
                    }
                }
            }
        }
    }
}

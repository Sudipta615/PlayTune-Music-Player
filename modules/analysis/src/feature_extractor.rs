use realfft::RealFftPlanner;
use std::f32::consts::PI;
use std::fs::File;
use std::path::Path;
use symphonia::core::{
    codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO},
    formats::{probe::Hint, FormatOptions, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::Timestamp,
};

use db::models::TrackAudioFeatures;

/// DSP Audio Feature Extractor for Mood Analysis.
pub struct AudioFeatureExtractor {
    pub sample_rate: u32,
    pub fft_size: usize,
    pub hop_size: usize,
    pub analysis_secs: u32,
}

impl Default for AudioFeatureExtractor {
    fn default() -> Self {
        Self { sample_rate: 22050, fft_size: 1024, hop_size: 512, analysis_secs: 30 }
    }
}

impl AudioFeatureExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_window_secs(secs: u32) -> Self {
        Self { analysis_secs: secs, ..Self::default() }
    }

    /// Extract feature vector from an audio file path.
    /// Decodes a 30-second middle window of the audio file to keep performance fast.
    pub fn extract_from_file(
        &self,
        track_id: i64,
        path: impl AsRef<Path>,
    ) -> Result<TrackAudioFeatures, String> {
        let file = File::open(path.as_ref()).map_err(|e| format!("Failed to open file: {}", e))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.as_ref().extension().and_then(|s| s.to_str()) {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = AudioDecoderOptions::default();

        let mut format = symphonia::default::get_probe()
            .probe(&hint, mss, format_opts, metadata_opts)
            .map_err(|e| format!("Symphonia probe failed: {}", e))?;

        let track = format
            .tracks()
            .iter()
            .find(|t| {
                t.codec_params
                    .as_ref()
                    .and_then(|cp| cp.audio())
                    .is_some_and(|a| a.codec != CODEC_ID_NULL_AUDIO)
            })
            .ok_or_else(|| "No default audio track found".to_string())?;

        let track_id_spec = track.id;
        let audio_params = track
            .codec_params
            .as_ref()
            .and_then(|cp| cp.audio())
            .ok_or_else(|| "No audio codec parameters found".to_string())?;

        let original_sample_rate = audio_params.sample_rate.unwrap_or(44100);
        let channels = audio_params.channels.as_ref().map_or(2, |c| c.count());

        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(audio_params, &decoder_opts)
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        // Target sample count at 22.05 kHz (approx 661,500 mono samples for 30s)
        let total_samples_target = (self.analysis_secs * self.sample_rate) as usize;
        let segment_samples_target = total_samples_target / 3;
        let mut raw_mono_samples: Vec<f32> = Vec::with_capacity(total_samples_target);
        let mut temp_interleaved: Vec<f32> = Vec::new();

        // 3-Point Multi-Window Sampling (20%, 50%, 80% timestamp windows)
        let n_frames = track.num_frames;
        if let Some(total_frames) = n_frames {
            let target_duration_frames = self.analysis_secs as u64 * original_sample_rate as u64;
            if total_frames > target_duration_frames {
                let p20 = (total_frames as f64 * 0.20) as u64;
                let p50 = (total_frames as f64 * 0.50) as u64;
                let p80 = (total_frames as f64 * 0.80) as u64;

                for ts in [p20, p50, p80] {
                    let _ = format.seek(
                        SeekMode::Accurate,
                        SeekTo::Timestamp {
                            ts: Timestamp::new(ts as i64),
                            track_id: track_id_spec,
                        },
                    );
                    let mut seg_count = 0;
                    while seg_count < segment_samples_target {
                        let packet = match format.next_packet() {
                            Ok(Some(p)) => p,
                            Ok(None) => break,
                            Err(symphonia::core::errors::Error::ResetRequired) => continue,
                            Err(_) => break,
                        };
                        if packet.track_id != track_id_spec {
                            continue;
                        }
                        let decoded = match decoder.decode(&packet) {
                            Ok(d) => d,
                            Err(_) => break,
                        };
                        temp_interleaved.clear();
                        decoded.copy_to_vec_interleaved(&mut temp_interleaved);
                        let samples = &temp_interleaved;
                        let step = (original_sample_rate as f32 / self.sample_rate as f32).max(1.0)
                            as usize;
                        let mut idx = 0;
                        while idx + channels <= samples.len() {
                            let mut sum = 0.0f32;
                            for c in 0..channels {
                                sum += samples[idx + c];
                            }
                            raw_mono_samples.push(sum / channels as f32);
                            seg_count += 1;
                            idx += channels * step;
                            if seg_count >= segment_samples_target {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Fallback to sequential decoding if 3-point sampling yielded insufficient samples
        if raw_mono_samples.is_empty() {
            while raw_mono_samples.len() < total_samples_target {
                let packet = match format.next_packet() {
                    Ok(Some(p)) => p,
                    Ok(None) => break,
                    Err(symphonia::core::errors::Error::ResetRequired) => continue,
                    Err(_) => break,
                };

                if packet.track_id != track_id_spec {
                    continue;
                }

                let decoded = match decoder.decode(&packet) {
                    Ok(d) => d,
                    Err(_) => break,
                };

                temp_interleaved.clear();
                decoded.copy_to_vec_interleaved(&mut temp_interleaved);
                let samples = &temp_interleaved;

                let step =
                    (original_sample_rate as f32 / self.sample_rate as f32).max(1.0) as usize;
                let mut idx = 0;
                while idx + channels <= samples.len() {
                    let mut sum = 0.0f32;
                    for c in 0..channels {
                        sum += samples[idx + c];
                    }
                    raw_mono_samples.push(sum / channels as f32);
                    idx += channels * step;
                    if raw_mono_samples.len() >= total_samples_target {
                        break;
                    }
                }
            }
        }

        if raw_mono_samples.is_empty() {
            return Err("Audio stream contained no decodable audio samples".to_string());
        }

        Ok(self.extract_from_samples(track_id, &raw_mono_samples))
    }

    /// Extract feature metrics directly from raw mono samples array.
    pub fn extract_from_samples(&self, track_id: i64, samples: &[f32]) -> TrackAudioFeatures {
        let n_samples = samples.len();
        if n_samples < self.fft_size {
            return TrackAudioFeatures { track_id, ..Default::default() };
        }

        // 0. Crest Factor (Peak to RMS Ratio)
        let max_abs = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // 1. RMS Energy & ZCR per frame
        let mut rms_values = Vec::new();
        let mut zcr_values = Vec::new();

        let mut offset = 0;
        while offset + self.fft_size <= n_samples {
            let frame = &samples[offset..offset + self.fft_size];

            // RMS
            let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
            let rms = (sum_sq / self.fft_size as f32).sqrt();
            rms_values.push(rms);

            // ZCR
            let mut zcr_count = 0;
            for i in 1..frame.len() {
                if (frame[i] >= 0.0 && frame[i - 1] < 0.0)
                    || (frame[i] < 0.0 && frame[i - 1] >= 0.0)
                {
                    zcr_count += 1;
                }
            }
            zcr_values.push(zcr_count as f32 / self.fft_size as f32);

            offset += self.hop_size;
        }

        let (rms_mean, rms_std) = mean_std(&rms_values);
        let (zcr_mean, zcr_std) = mean_std(&zcr_values);

        let crest_factor = if rms_mean > 1e-7 { max_abs / rms_mean } else { 1.0 };

        // 2. STFT Spectral Features
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(self.fft_size);
        let mut scratch = fft.make_scratch_vec();
        let mut fft_output = fft.make_output_vec();

        // Hann window
        let window: Vec<f32> = (0..self.fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (self.fft_size - 1) as f32).cos()))
            .collect();

        let num_bins = (self.fft_size / 2) + 1;
        let bin_freqs: Vec<f32> = (0..num_bins)
            .map(|i| i as f32 * self.sample_rate as f32 / self.fft_size as f32)
            .collect();

        let mut centroids = Vec::new();
        let mut rolloffs = Vec::new();
        let mut flatnesses = Vec::new();
        let mut fluxes = Vec::new();
        let mut spectral_contrasts = Vec::new();
        let mut mfccs_frames: Vec<Vec<f32>> = Vec::new();
        let mut chroma_accum = vec![0.0f32; 12];
        let mut mag_frames: Vec<Vec<f32>> = Vec::new();

        let mel_filters = create_mel_filterbank(20, num_bins, self.sample_rate as f32);

        let mut prev_mag = vec![0.0f32; num_bins];
        let mut input_buf = vec![0.0f32; self.fft_size];

        offset = 0;
        let mut frame_count = 0;

        // Sub-band frequency bin bounds for Spectral Contrast (6 bands)
        let subband_bounds = [
            (0, (num_bins / 16).max(2)),
            ((num_bins / 16).max(2), (num_bins / 8).max(4)),
            ((num_bins / 8).max(4), (num_bins / 4).max(8)),
            ((num_bins / 4).max(8), (num_bins / 2).max(16)),
            ((num_bins / 2).max(16), (num_bins * 3 / 4).max(32)),
            ((num_bins * 3 / 4).max(32), num_bins),
        ];

        while offset + self.fft_size <= n_samples {
            for i in 0..self.fft_size {
                input_buf[i] = samples[offset + i] * window[i];
            }

            if fft.process_with_scratch(&mut input_buf, &mut fft_output, &mut scratch).is_ok() {
                let mut mag = vec![0.0f32; num_bins];
                let mut sum_mag = 0.0f32;
                let mut weighted_sum = 0.0f32;
                let mut log_sum = 0.0f32;

                for i in 0..num_bins {
                    let m = (fft_output[i].re * fft_output[i].re
                        + fft_output[i].im * fft_output[i].im)
                        .sqrt();
                    mag[i] = m;
                    sum_mag += m;
                    weighted_sum += bin_freqs[i] * m;
                    log_sum += (m + 1e-9).ln();
                }

                mag_frames.push(mag.clone());

                // Centroid
                let centroid = if sum_mag > 1e-7 { weighted_sum / sum_mag } else { 0.0 };
                centroids.push(centroid);

                // Rolloff (85% energy threshold)
                let threshold = 0.85 * sum_mag;
                let mut cum_sum = 0.0f32;
                let mut rolloff = 0.0f32;
                for i in 0..num_bins {
                    cum_sum += mag[i];
                    if cum_sum >= threshold {
                        rolloff = bin_freqs[i];
                        break;
                    }
                }
                rolloffs.push(rolloff);

                // Flatness
                let geom_mean = (log_sum / num_bins as f32).exp();
                let arith_mean = sum_mag / num_bins as f32;
                let flatness = if arith_mean > 1e-7 { geom_mean / arith_mean } else { 0.0 };
                flatnesses.push(flatness);

                // Flux
                let mut flux = 0.0f32;
                for i in 0..num_bins {
                    let diff = mag[i] - prev_mag[i];
                    if diff > 0.0 {
                        flux += diff * diff;
                    }
                }
                fluxes.push(flux.sqrt());
                prev_mag = mag.clone();

                // Spectral Contrast per sub-band
                let mut contrast_sum = 0.0f32;
                for &(b_start, b_end) in &subband_bounds {
                    if b_start < b_end && b_end <= num_bins {
                        let mut band_mags: Vec<f32> = mag[b_start..b_end].to_vec();
                        band_mags
                            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        let k = (band_mags.len() / 5).max(1);
                        let valley_avg: f32 = band_mags[..k].iter().sum::<f32>() / k as f32;
                        let peak_avg: f32 =
                            band_mags[band_mags.len() - k..].iter().sum::<f32>() / k as f32;
                        let contrast = ((peak_avg + 1e-6) / (valley_avg + 1e-6)).ln();
                        contrast_sum += contrast;
                    }
                }
                spectral_contrasts.push(contrast_sum / subband_bounds.len() as f32);

                // MFCC calculation (20 mel bands -> 13 DCT coefficients)
                let mut mel_energies = Vec::with_capacity(20);
                for filter in &mel_filters {
                    let mut e = 0.0f32;
                    for &(bin, weight) in filter {
                        if bin < num_bins {
                            e += mag[bin] * weight;
                        }
                    }
                    mel_energies.push((e + 1e-7).ln());
                }

                // DCT-II for 13 MFCCs
                let mut mfcc = vec![0.0f32; 13];
                for (k, mfcc_k) in mfcc.iter_mut().enumerate().take(13) {
                    let mut sum = 0.0f32;
                    for (n, &mel_e) in mel_energies.iter().enumerate().take(20) {
                        sum += mel_e * (PI * k as f32 * (n as f32 + 0.5) / 20.0).cos();
                    }
                    *mfcc_k = sum;
                }
                mfccs_frames.push(mfcc);

                // Chromagram
                for i in 1..num_bins {
                    let f = bin_freqs[i];
                    if (65.0..=4200.0).contains(&f) {
                        let note = 69.0 + 12.0 * (f / 440.0).log2();
                        let pitch_class = (note.round() as i32).rem_euclid(12) as usize;
                        chroma_accum[pitch_class] += mag[i];
                    }
                }

                frame_count += 1;
            }

            offset += self.hop_size;
        }

        // 3. Harmonic vs Percussive Ratio (HPR) via median filtering on STFT matrix
        let hpr = if !mag_frames.is_empty() {
            let n_f = mag_frames.len();
            let mut harmonic_energy = 0.0f32;
            let mut percussive_energy = 0.0f32;

            for t in 0..n_f {
                for b in 0..num_bins {
                    // Harmonic: median along time axis (stack-allocated window up to 5)
                    let t_start = t.saturating_sub(2);
                    let t_end = (t + 3).min(n_f);
                    let mut time_buf = [0.0f32; 5];
                    let time_len = t_end - t_start;
                    for (idx, i) in (t_start..t_end).enumerate() {
                        time_buf[idx] = mag_frames[i][b];
                    }
                    let time_slice = &mut time_buf[..time_len];
                    time_slice
                        .sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                    let h_val = time_slice[time_len / 2];

                    // Percussive: median along frequency axis (stack-allocated window up to 5)
                    let b_start = b.saturating_sub(2);
                    let b_end = (b + 3).min(num_bins);
                    let mut freq_buf = [0.0f32; 5];
                    let freq_len = b_end - b_start;
                    freq_buf[..freq_len].copy_from_slice(&mag_frames[t][b_start..b_end]);
                    let freq_slice = &mut freq_buf[..freq_len];
                    freq_slice
                        .sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                    let p_val = freq_slice[freq_len / 2];

                    harmonic_energy += h_val;
                    percussive_energy += p_val;
                }
            }
            (harmonic_energy + 1e-6) / (percussive_energy + 1e-6)
        } else {
            1.0
        };

        let (spectral_centroid_mean, spectral_centroid_std) = mean_std(&centroids);
        let (spectral_rolloff_mean, spectral_rolloff_std) = mean_std(&rolloffs);
        let (spectral_flatness_mean, spectral_flatness_std) = mean_std(&flatnesses);
        let (spectral_flux_mean, spectral_flux_std) = mean_std(&fluxes);
        let (spectral_contrast_mean, spectral_contrast_std) = mean_std(&spectral_contrasts);

        // Major vs Minor Mode Ratio from Chromagram
        let mut max_major = 0.0f32;
        let mut max_minor = 0.0f32;
        for r in 0..12 {
            let maj = chroma_accum[r] + chroma_accum[(r + 4) % 12] + chroma_accum[(r + 7) % 12];
            let min = chroma_accum[r] + chroma_accum[(r + 3) % 12] + chroma_accum[(r + 7) % 12];
            if maj > max_major {
                max_major = maj;
            }
            if min > max_minor {
                max_minor = min;
            }
        }
        let mode_major_ratio = (max_major + 1e-6) / (max_minor + 1e-6);

        // Summarize 13 MFCCs into means and stds
        let mut mfcc_summary = Vec::with_capacity(13);
        for c in 0..13 {
            let col_vals: Vec<f32> = mfccs_frames.iter().map(|frame| frame[c]).collect();
            let (m, s) = mean_std(&col_vals);
            mfcc_summary.push((m, s));
        }

        // Normalize Chromagram
        if frame_count > 0 {
            for val in chroma_accum.iter_mut().take(12) {
                *val /= frame_count as f32;
            }
            let max_c = chroma_accum.iter().copied().fold(0.0f32, f32::max);
            if max_c > 1e-7 {
                for val in chroma_accum.iter_mut().take(12) {
                    *val /= max_c;
                }
            }
        }

        // Simple Tempo Estimation via onset energy autocorrelation
        let tempo = estimate_tempo(&rms_values, self.sample_rate as f32 / self.hop_size as f32);

        TrackAudioFeatures {
            track_id,
            tempo,
            rms_mean,
            rms_std,
            zcr_mean,
            zcr_std,
            spectral_centroid_mean,
            spectral_centroid_std,
            spectral_rolloff_mean,
            spectral_rolloff_std,
            spectral_flatness_mean,
            spectral_flatness_std,
            spectral_flux_mean,
            spectral_flux_std,
            hpr,
            spectral_contrast_mean,
            spectral_contrast_std,
            crest_factor,
            mode_major_ratio,
            mfcc_json: serde_json::to_string(&mfcc_summary).unwrap_or_default(),
            chroma_json: serde_json::to_string(&chroma_accum).unwrap_or_default(),
        }
    }
}

/// Calculate Mean and Standard Deviation of a slice of f32 values.
fn mean_std(vals: &[f32]) -> (f32, f32) {
    if vals.is_empty() {
        return (0.0, 0.0);
    }
    let mean = vals.iter().sum::<f32>() / vals.len() as f32;
    let var = vals.iter().map(|&v| (v - mean) * (v - mean)).sum::<f32>() / vals.len() as f32;
    (mean, var.sqrt())
}

/// Construct triangular Mel-filterbank filters.
fn create_mel_filterbank(
    n_filters: usize,
    n_fft_bins: usize,
    sample_rate: f32,
) -> Vec<Vec<(usize, f32)>> {
    let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f32| 700.0 * (10.0f32.powf(mel / 2595.0) - 1.0);

    let min_mel = hz_to_mel(150.0);
    let max_mel = hz_to_mel(sample_rate / 2.0);

    let mel_points: Vec<f32> = (0..=n_filters + 1)
        .map(|i| min_mel + i as f32 * (max_mel - min_mel) / (n_filters + 1) as f32)
        .collect();

    let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();
    let bin_points: Vec<usize> = hz_points
        .iter()
        .map(|&h| {
            ((n_fft_bins as f32 * h / (sample_rate / 2.0)).floor() as usize).min(n_fft_bins - 1)
        })
        .collect();

    let mut filters = Vec::with_capacity(n_filters);

    for m in 1..=n_filters {
        let b_prev = bin_points[m - 1];
        let b_curr = bin_points[m];
        let b_next = bin_points[m + 1];

        let mut filter = Vec::new();

        for b in b_prev..=b_curr {
            if b_curr > b_prev {
                let weight = (b - b_prev) as f32 / (b_curr - b_prev) as f32;
                filter.push((b, weight));
            }
        }
        for b in b_curr + 1..=b_next {
            if b_next > b_curr {
                let weight = (b_next - b) as f32 / (b_next - b_curr) as f32;
                filter.push((b, weight));
            }
        }

        filters.push(filter);
    }

    filters
}

/// Estimate BPM from RMS energy onset frame envelope via peak autocorrelation.
fn estimate_tempo(rms_envelope: &[f32], frame_rate: f32) -> f32 {
    if rms_envelope.len() < 50 {
        return 120.0;
    }

    // Onset strength (first difference)
    let mut diffs = Vec::with_capacity(rms_envelope.len());
    diffs.push(0.0);
    for i in 1..rms_envelope.len() {
        let d = rms_envelope[i] - rms_envelope[i - 1];
        diffs.push(if d > 0.0 { d } else { 0.0 });
    }

    // Autocorrelation for lag range corresponding to 60 BPM - 180 BPM
    // lag_frames = (60 / BPM) * frame_rate
    let min_bpm = 60.0f32;
    let max_bpm = 180.0f32;

    let min_lag = (60.0 / max_bpm * frame_rate) as usize;
    let max_lag = (60.0 / min_bpm * frame_rate) as usize;

    let mut best_lag = min_lag;
    let mut max_corr = -1.0f32;

    for lag in min_lag..=max_lag.min(diffs.len() / 2) {
        let mut corr = 0.0f32;
        for i in 0..diffs.len() - lag {
            corr += diffs[i] * diffs[i + lag];
        }
        if corr > max_corr {
            max_corr = corr;
            best_lag = lag;
        }
    }

    if best_lag > 0 {
        (60.0 * frame_rate / best_lag as f32).clamp(60.0, 180.0)
    } else {
        120.0
    }
}

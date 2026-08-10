pub mod feature_extractor;
pub mod mood_classifier;

pub use feature_extractor::AudioFeatureExtractor;
pub use mood_classifier::MoodClassifierModel;

#[derive(Debug, Clone, Default)]
pub struct TrackAnalysis {
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub waveform: Vec<f32>,
}

//
// TODO: implement BpmDetector::analyze using an autocorrelation or
// onset-detection algorithm. Implement WaveformGenerator::generate using
// per-bucket peak/RMS reduction of the input samples.
#[derive(Debug, Clone, Default)]
pub struct BpmDetector {
    pub sample_rate: u32,
}

impl BpmDetector {
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    pub fn analyze(&self, _samples: &[f32]) -> Option<f32> {
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct WaveformGenerator {
    pub points: usize,
}

impl WaveformGenerator {
    pub fn new(points: usize) -> Self {
        Self { points }
    }

    pub fn generate(&self, _samples: &[f32]) -> Vec<f32> {
        vec![0.0; self.points]
    }
}

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct FftVisualizerTap {
    inner: Arc<FftVisualizerTapInner>,
}

struct FftVisualizerTapInner {
    input_mutex: Mutex<FftInput>,
    dirty: AtomicBool,
    output_mutex: Mutex<FftOutput>,
    fft: Arc<dyn RealToComplex<f32>>,
    windowed_buf: Mutex<Vec<f32>>,
    scratch_buf: Mutex<Vec<Complex<f32>>>,
    output_buf: Mutex<Vec<Complex<f32>>>,
    window: Vec<f32>,
    bar_count: usize,
    fft_size: usize,
    bar_bins: Vec<(usize, usize)>,
    frames_since_last_fft: AtomicU32,
}

struct FftInput {
    /// Pre-allocated ring buffer of mono samples. Fixed capacity.
    buf: Vec<f32>,
    /// Write position (next slot to write). Wraps with `buf.len()`.
    write_pos: usize,
    /// Number of valid samples currently in the ring (0..=buf.len()).
    len: usize,
}

struct FftOutput {
    smoothed_bars: Vec<f32>,
}

impl FftVisualizerTap {
    pub fn new(fft_size: usize, bar_count: usize) -> Self {
        let fft_size = fft_size.max(2);
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let scratch_buf = fft.make_scratch_vec();
        let output_buf = fft.make_output_vec();

        let mut window = Vec::with_capacity(fft_size);
        for i in 0..fft_size {
            let frac = i as f32 / (fft_size - 1) as f32;
            let val = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * frac).cos());
            window.push(val);
        }

        // Pre-allocate the input ring to 4× the FFT size. This gives the
        // GUI thread ~4 FFT windows of headroom before samples are
        // overwritten. At 44.1 kHz / 512-sample FFT, that's ~46 ms of
        // audio — plenty for a 33 Hz GUI update rate.
        let input_capacity = fft_size * 4;
        let bins = (fft_size / 2) + 1;
        let max_bin = (bins - 1) as f32;
        let mut bar_bins = Vec::with_capacity(bar_count);
        for b in 0..bar_count {
            let t_start = b as f32 / bar_count as f32;
            let t_end = (b + 1) as f32 / bar_count as f32;
            let start_bin = max_bin.powf(t_start).round() as usize;
            let end_bin = max_bin.powf(t_end).round() as usize;
            let start_bin = start_bin.clamp(1, bins - 1);
            let end_bin = end_bin.clamp(start_bin + 1, bins);
            bar_bins.push((start_bin, end_bin));
        }

        Self {
            inner: Arc::new(FftVisualizerTapInner {
                input_mutex: Mutex::new(FftInput {
                    buf: vec![0.0; input_capacity],
                    write_pos: 0,
                    len: 0,
                }),
                dirty: AtomicBool::new(false),
                output_mutex: Mutex::new(FftOutput { smoothed_bars: vec![0.0; bar_count] }),
                fft,
                windowed_buf: Mutex::new(vec![0.0; fft_size]),
                scratch_buf: Mutex::new(scratch_buf),
                output_buf: Mutex::new(output_buf),
                window,
                bar_count,
                fft_size,
                bar_bins,
                frames_since_last_fft: AtomicU32::new(0),
            }),
        }
    }

    /// Push audio samples into the visualizer. Called from the real-time
    /// audio callback. Zero-allocation, zero-FFT, try-lock (bails instantly
    /// on contention).
    pub fn process_samples(&self, samples: &[f32], channels: usize) {
        if samples.is_empty() {
            return;
        }
        let mut input = match self.inner.input_mutex.try_lock() {
            Ok(guard) => guard,
            Err(_) => return, // GUI thread is draining — drop these samples.
        };

        let ch = if channels == 0 { 1 } else { channels };
        let cap = input.buf.len();
        if ch == 2 {
            let mut idx = 0;
            while idx + 1 < samples.len() {
                let sample = (samples[idx] + samples[idx + 1]) * 0.5;
                let wp = input.write_pos;
                input.buf[wp] = sample;
                input.write_pos = (wp + 1) % cap;
                if input.len < cap {
                    input.len += 1;
                }
                idx += 2;
            }
        } else {
            let mut idx = 0;
            while idx < samples.len() {
                let mut sum = 0.0f32;
                let mut count = 0;
                for c in 0..ch {
                    if idx + c < samples.len() {
                        sum += samples[idx + c];
                        count += 1;
                    }
                }
                if count > 0 {
                    let sample = sum / count as f32;
                    // Write into the ring buffer, overwriting the oldest sample
                    // if full. This maintains a sliding window of the most
                    // recent `cap` samples.
                    let wp = input.write_pos;
                    input.buf[wp] = sample;
                    input.write_pos = (wp + 1) % cap;
                    if input.len < cap {
                        input.len += 1;
                    }
                }
                idx += ch;
            }
        }

        // Signal the GUI thread that new samples are available.
        self.inner.dirty.store(true, Ordering::Release);
        self.inner.frames_since_last_fft.fetch_add(1, Ordering::Relaxed);
    }

    /// Run the FFT + bar mapping if new samples are available, then return
    /// the smoothed bars. Called from the GUI ticker thread (~33 Hz).
    fn maybe_run_fft(&self) {
        // Quick check without taking any lock.
        if !self.inner.dirty.load(Ordering::Acquire) {
            return;
        }
        let fft_size = self.inner.fft_size;
        let bar_count = self.inner.bar_count;

        // Read samples from the input ring directly into the pre-allocated
        // windowed_buf (avoids a per-call Vec allocation). We hold both
        // input_mutex and windowed_buf lock briefly — no deadlock risk
        // because no other code path locks both (audio callback only
        // touches input_mutex).
        let mut windowed_buf = match self.inner.windowed_buf.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        {
            let input = match self.inner.input_mutex.try_lock() {
                Ok(g) => g,
                Err(_) => return, // Audio callback is pushing — try next tick.
            };
            if input.len < fft_size {
                // Not enough samples yet — skip FFT.
                // Don't clear `dirty` so we try again next tick.
                return;
            }
            let cap = input.buf.len();
            // Read the most recent `fft_size` samples in chronological order.
            // start = (write_pos - fft_size + cap) % cap
            let start = (input.write_pos + cap - fft_size) % cap;
            for i in 0..fft_size {
                windowed_buf[i] = input.buf[(start + i) % cap] * self.inner.window[i];
            }
        } // input_mutex released

        // Run the FFT. This is the expensive part (~5-20 µs for a 512-point
        // real FFT), but it's now on the GUI thread, not the RT thread.
        let fft = self.inner.fft.clone();
        let mut output_buf = match self.inner.output_buf.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let mut scratch_buf = match self.inner.scratch_buf.lock() {
            Ok(g) => g,
            Err(_) => {
                drop(output_buf);
                return;
            }
        };
        let _ = fft.process_with_scratch(&mut windowed_buf, &mut output_buf, &mut scratch_buf);
        // Release windowed_buf and scratch_buf — we only need output_buf
        // for the bar mapping below.
        drop(windowed_buf);
        drop(scratch_buf);

        // Map spectrum to frequency bars (logarithmic spacing).
        let bins = output_buf.len();
        if bins < 2 {
            return;
        }

        // Compute new bars into a stack buffer (65 bars × 4 bytes = 260 bytes,
        // well within stack limits). Then merge under the output lock.
        let mut new_bars = [0.0f32; 128]; // generous upper bound for bar_count
        debug_assert!(bar_count <= new_bars.len(), "bar_count {} exceeds stack buffer", bar_count);
        let bar_count = bar_count.min(new_bars.len());
        let bar_bins = &self.inner.bar_bins;
        for b in 0..bar_count {
            // Bounds-check the lookup table once. If the table was built for
            // a different `bar_count` (shouldn't happen — constructor runs
            // once), fall back to the old computation.
            let (start_bin, end_bin) = if b < bar_bins.len() {
                bar_bins[b]
            } else {
                let max_bin = (bins - 1) as f32;
                let t_start = b as f32 / bar_count as f32;
                let t_end = (b + 1) as f32 / bar_count as f32;
                let s = max_bin.powf(t_start).round() as usize;
                let e = max_bin.powf(t_end).round() as usize;
                (s.clamp(1, bins - 1), e.clamp(s + 1, bins))
            };

            let mut peak = 0.0f32;
            for i in start_bin..end_bin {
                let c = output_buf[i];
                let mag2 = c.re * c.re + c.im * c.im;
                if !mag2.is_finite() {
                    continue;
                }
                let mag = mag2.sqrt() / (fft_size as f32 * 0.15);
                if mag > peak {
                    peak = mag;
                }
            }

            let tilt = 1.0 + (b as f32 / bar_count as f32) * 1.5;
            let raw = if peak.is_finite() && peak > 0.0 {
                (peak * tilt * 4.0).powf(0.7).clamp(0.05, 0.98)
            } else {
                0.05
            };
            new_bars[b] = raw;
        }
        drop(output_buf);

        let mut output = match self.inner.output_mutex.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        for (slot, &raw) in
            output.smoothed_bars.iter_mut().take(bar_count).zip(new_bars.iter().take(bar_count))
        {
            if raw > *slot {
                *slot = *slot * 0.4 + raw * 0.6; // fast attack
            } else {
                *slot = *slot * 0.8 + raw * 0.2; // smooth release
            }
        }
        drop(output);

        // Clear the dirty flag — we've consumed the new samples.
        self.inner.dirty.store(false, Ordering::Release);
        self.inner.frames_since_last_fft.store(0, Ordering::Release);
    }

    pub fn get_bars(&self) -> Vec<f32> {
        // Run the FFT if new samples are available (moves the FFT off the
        // RT thread — see process_samples docstring).
        self.maybe_run_fft();
        match self.inner.output_mutex.lock() {
            Ok(guard) => guard.smoothed_bars.clone(),
            Err(_) => Vec::new(),
        }
    }

    ///
    /// If `out.len()` is smaller than the number of bars, only the first
    /// `out.len()` bars are written. If `out.len()` is larger, the extra
    /// slots are left untouched (callers should pre-size the buffer to
    /// the expected bar count, typically 16-64).
    pub fn get_bars_into(&self, out: &mut [f32]) -> usize {
        self.maybe_run_fft();
        match self.inner.output_mutex.lock() {
            Ok(guard) => {
                let n = guard.smoothed_bars.len().min(out.len());
                out[..n].copy_from_slice(&guard.smoothed_bars[..n]);
                n
            }
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_extraction_synthetic() {
        let extractor = AudioFeatureExtractor::new();

        // Generate 3 seconds of 440 Hz sine wave @ 22050 Hz
        let sample_rate = 22050;
        let num_samples = 3 * sample_rate;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            samples.push(val);
        }

        let features = extractor.extract_from_samples(42, &samples);
        assert_eq!(features.track_id, 42);
        assert!(features.rms_mean > 0.3); // RMS of sine wave 0.5 amp is ~0.353
        assert!(features.spectral_centroid_mean > 350.0 && features.spectral_centroid_mean < 550.0); // Close to 440 Hz

        // Check MFCC & Chroma JSON formatting
        let mfcc_parsed: Vec<(f32, f32)> = serde_json::from_str(&features.mfcc_json).unwrap();
        assert_eq!(mfcc_parsed.len(), 13);

        let chroma_parsed: Vec<f32> = serde_json::from_str(&features.chroma_json).unwrap();
        assert_eq!(chroma_parsed.len(), 12);
    }

    #[test]
    fn test_mood_classifier_tree_eval() {
        use mood_classifier::{MoodClassifierModel, MoodEnsemble, Tree, TreeNode};

        let tree = Tree {
            nodes: vec![
                TreeNode {
                    feature_idx: 0, // tempo
                    threshold: 120.0,
                    left_child: Some(1),
                    right_child: Some(2),
                    leaf_value: 0.0,
                    is_leaf: false,
                },
                TreeNode {
                    feature_idx: 0,
                    threshold: 0.0,
                    left_child: None,
                    right_child: None,
                    leaf_value: -1.5,
                    is_leaf: true,
                },
                TreeNode {
                    feature_idx: 0,
                    threshold: 0.0,
                    left_child: None,
                    right_child: None,
                    leaf_value: 2.0,
                    is_leaf: true,
                },
            ],
        };

        let ensemble = MoodEnsemble { trees: vec![tree], base_score: 0.0 };

        let model = MoodClassifierModel { energetic: ensemble, ..Default::default() };

        let low_tempo_features =
            db::models::TrackAudioFeatures { track_id: 1, tempo: 90.0, ..Default::default() };
        let scores_low = model.classify(&low_tempo_features);
        assert!(scores_low.energetic < 0.5); // Sigmoid(-1.5) approx 0.18

        let high_tempo_features =
            db::models::TrackAudioFeatures { track_id: 2, tempo: 140.0, ..Default::default() };
        let scores_high = model.classify(&high_tempo_features);
        assert!(scores_high.energetic > 0.8); // Sigmoid(2.0) approx 0.88
    }
}

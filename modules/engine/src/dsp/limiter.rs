//! Lookahead brick-wall limiter
//!
//! Prevents the output from exceeding a configurable ceiling by looking ahead
//! in the signal and applying smooth gain reduction. Supports soft clipping
//! as an optional safety mode.

use crate::buffer::{AudioFrame, MAX_CHANNELS};

/// Lookahead brick-wall limiter
pub struct LookaheadLimiter {
    /// Ceiling in linear amplitude
    ceiling_linear: f32,
    /// Attack time in seconds
    attack_secs: f32,
    /// Release time in seconds
    release_secs: f32,
    /// Lookahead time in seconds (stored for sample rate changes)
    lookahead_secs: f32,
    /// Lookahead delay in samples
    lookahead_samples: usize,
    /// Sample rate
    sample_rate: f32,
    /// Whether soft clipping is enabled
    soft_clip: bool,
    /// Whether the limiter is enabled
    enabled: bool,

    /// Delay line per channel — circular buffer
    delay_lines: [Vec<f32>; MAX_CHANNELS],
    /// Write position in the delay line
    delay_write_pos: usize,
    /// Current gain reduction (linear, 0.0–1.0)
    current_gain: f32,
    /// Attack coefficient (per sample)
    attack_coeff: f32,
    /// Release coefficient (per sample)
    release_coeff: f32,
}

impl LookaheadLimiter {
    /// Create a new limiter with full configuration
    pub fn new_with_params(
        sample_rate: f32,
        lookahead_ms: f32,
        attack_ms: f32,
        release_ms: f32,
        ceiling_db: f32,
        soft_clip: bool,
    ) -> Self {
        let lookahead_secs = lookahead_ms / 1000.0;
        let lookahead_samples = (lookahead_secs * sample_rate).ceil() as usize;
        let lookahead_samples = lookahead_samples.max(1);
        let attack_secs = attack_ms / 1000.0;
        let release_secs = release_ms / 1000.0;
        let clamped_ceiling_db =
            if ceiling_db.is_finite() { ceiling_db.clamp(-60.0, 0.0) } else { -0.3 };
        let ceiling_linear = 10.0_f32.powf(clamped_ceiling_db / 20.0);

        let attack_coeff =
            if attack_secs > 0.0 { (-1.0 / (attack_secs * sample_rate)).exp() } else { 0.0 };
        let release_coeff =
            if release_secs > 0.0 { (-1.0 / (release_secs * sample_rate)).exp() } else { 0.0 };

        Self {
            ceiling_linear,
            attack_secs,
            release_secs,
            lookahead_secs,
            lookahead_samples,
            sample_rate,
            soft_clip,
            enabled: true,
            delay_lines: [
                vec![0.0; (lookahead_samples + 1).next_power_of_two()],
                vec![0.0; (lookahead_samples + 1).next_power_of_two()],
            ],
            delay_write_pos: 0,
            current_gain: 1.0,
            attack_coeff,
            release_coeff,
        }
    }

    /// Create a new limiter with default settings
    pub fn new(sample_rate: f32) -> Self {
        Self::new_with_params(sample_rate, 5.0, 5.0, 50.0, -0.3, false)
    }

    /// Enable or disable the limiter
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set the ceiling in dB. Must be <= 0 dBFS; non-finite or positive values are clamped.
    pub fn set_ceiling_db(&mut self, ceiling_db: f32) {
        if !ceiling_db.is_finite() {
            log::warn!("LookaheadLimiter: non-finite ceiling_db {}; using -0.3", ceiling_db);
            self.ceiling_linear = 10.0_f32.powf(-0.3 / 20.0);
            return;
        }
        let clamped_db = ceiling_db.clamp(-60.0, 0.0);
        self.ceiling_linear = 10.0_f32.powf(clamped_db / 20.0);
    }

    /// Set the threshold in dB (alias for ceiling)
    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.set_ceiling_db(threshold_db);
    }

    /// Set attack time in **milliseconds**. Non-positive/non-finite values
    /// are clamped to 0.1 ms.
    ///
    /// Bug fix (unit mismatch): the previous implementation treated the
    /// argument as seconds, but every caller in `pipeline.rs::set_limiter_params`
    /// passed `config.limiter.attack_ms` (a value in milliseconds). With
    /// `attack_ms = 5.0`, the old code computed
    /// `attack_coeff = exp(-1 / (5.0 * 44100))` ≈ 0.9999955, i.e. an attack
    /// time constant of ~5 SECONDS instead of 5 ms. The limiter never
    /// actually attacked, allowing peaks through unchanged. The new code
    /// converts ms → s internally, matching the constructor's units.
    pub fn set_attack(&mut self, attack_ms: f32) {
        if !attack_ms.is_finite() || attack_ms <= 0.0 {
            log::warn!("LookaheadLimiter: invalid attack {} ms; clamping to 0.1 ms", attack_ms);
            self.attack_secs = 0.0001;
        } else {
            self.attack_secs = attack_ms / 1000.0;
        }
        self.attack_coeff = (-1.0 / (self.attack_secs * self.sample_rate)).exp();
    }

    /// Set release time in **milliseconds**. Non-positive/non-finite values
    /// are clamped to 1 ms.
    ///
    /// Bug fix (unit mismatch): see `set_attack` — the previous implementation
    /// treated the argument as seconds while callers passed milliseconds.
    pub fn set_release(&mut self, release_ms: f32) {
        if !release_ms.is_finite() || release_ms <= 0.0 {
            log::warn!("LookaheadLimiter: invalid release {} ms; clamping to 1 ms", release_ms);
            self.release_secs = 0.001;
        } else {
            self.release_secs = release_ms / 1000.0;
        }
        self.release_coeff = (-1.0 / (self.release_secs * self.sample_rate)).exp();
    }

    /// Process a stereo sample pair
    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if !self.enabled {
            return (left, right);
        }
        let (left, right) = if left.is_nan() || right.is_nan() {
            if log::log_enabled!(log::Level::Error) {
                log::error!("LookaheadLimiter: NaN input detected, substituting 0.0");
            }
            (if left.is_nan() { 0.0 } else { left }, if right.is_nan() { 0.0 } else { right })
        } else {
            (left, right)
        };

        let input_peak = left.abs().max(right.abs());

        let desired_gain =
            if input_peak > self.ceiling_linear { self.ceiling_linear / input_peak } else { 1.0 };

        if desired_gain < self.current_gain {
            // Attack
            self.current_gain =
                desired_gain + (self.current_gain - desired_gain) * self.attack_coeff;
        } else {
            self.current_gain =
                desired_gain + (self.current_gain - desired_gain) * self.release_coeff;
        }
        self.current_gain = crate::buffer::flush_denormal(self.current_gain);
        self.current_gain = self.current_gain.clamp(0.0, 1.0);

        // giving exactly lookahead_samples of delay
        let delay_len = self.delay_lines[0].len();
        let read_pos =
            (self.delay_write_pos + delay_len - self.lookahead_samples) & (delay_len - 1);

        let delayed_left = self.delay_lines[0][read_pos];
        let delayed_right = self.delay_lines[1][read_pos];

        self.delay_lines[0][self.delay_write_pos] = left;
        self.delay_lines[1][self.delay_write_pos] = right;

        self.delay_write_pos = (self.delay_write_pos + 1) & (delay_len - 1);

        let mut out_l = delayed_left * self.current_gain;
        let mut out_r = delayed_right * self.current_gain;

        // Smooth peak compliance: apply soft-knee saturation rather than
        // abrupt sample-level gain multiplication to eliminate tearing/choppy distortion.
        let out_peak = out_l.abs().max(out_r.abs());
        if out_peak > self.ceiling_linear || self.soft_clip {
            out_l = self.soft_clip_sample(out_l);
            out_r = self.soft_clip_sample(out_r);
        }

        (out_l, out_r)
    }

    /// Process an audio frame (alternative API)
    pub fn process_frame(&mut self, frame: &mut AudioFrame) {
        let (l, r) = self.process(frame.channels[0], frame.channels[1]);
        frame.channels[0] = l;
        frame.channels[1] = r;
    }

    /// Soft clip using a smooth knee
    #[inline]
    fn soft_clip_sample(&self, sample: f32) -> f32 {
        let abs_sample = sample.abs();
        let limit = self.ceiling_linear;
        let threshold = limit * 0.8;

        if abs_sample <= threshold {
            return sample;
        }

        // Smooth knee from threshold asymptotically approaching limit
        let over = abs_sample - threshold;
        let range = limit - threshold;
        let saturated = threshold + range * (1.0 - (-over / range).exp());

        sample.signum() * saturated
    }

    /// Get the current gain reduction in dB (always <= 0)
    pub fn gain_reduction_db(&self) -> f32 {
        if self.current_gain > 0.0 {
            (20.0 * self.current_gain.log10()).max(-60.0)
        } else {
            -60.0
        }
    }

    /// Get the current gain (linear)
    pub fn current_gain(&self) -> f32 {
        self.current_gain
    }

    /// Reset the limiter state
    pub fn reset(&mut self) {
        for line in &mut self.delay_lines {
            line.fill(0.0);
        }
        self.delay_write_pos = 0;
        self.current_gain = 1.0;
    }

    /// Update sample rate (rebuilds delay lines)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let lookahead_samples = (self.lookahead_secs * sample_rate).ceil() as usize;
        self.lookahead_samples = lookahead_samples.max(1);
        for line in &mut self.delay_lines {
            line.resize((self.lookahead_samples + 1).next_power_of_two(), 0.0);
            line.fill(0.0);
        }
        self.delay_write_pos = 0;
        self.current_gain = 1.0;
        self.attack_coeff = (-1.0 / (self.attack_secs * self.sample_rate)).exp();
        self.release_coeff = (-1.0 / (self.release_secs * self.sample_rate)).exp();
    }

    pub fn set_lookahead(&mut self, ms: f32) {
        self.lookahead_secs = ms / 1000.0;
        self.set_sample_rate(self.sample_rate);
    }

    pub fn set_soft_clip(&mut self, soft_clip: bool) {
        self.soft_clip = soft_clip;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_prevents_clipping() {
        let mut limiter = LookaheadLimiter::new(44100.0);
        for _ in 0..1000 {
            let (l, r) = limiter.process(1.5, 1.5);
            // After delay fills, output should be controlled
            assert!(l.abs() <= 1.5);
            assert!(r.abs() <= 1.5);
        }
    }

    #[test]
    fn test_limiter_passes_quiet_signal() {
        let mut limiter = LookaheadLimiter::new(44100.0);
        for _ in 0..1000 {
            let (_l, _r) = limiter.process(0.1, 0.1);
        }
        let (l, r) = limiter.process(0.1, 0.1);
        assert!((l - 0.1).abs() < 0.05, "Quiet signal should pass through");
        assert!((r - 0.1).abs() < 0.05, "Quiet signal should pass through");
    }

    #[test]
    fn test_limiter_with_params() {
        let mut limiter = LookaheadLimiter::new_with_params(44100.0, 5.0, 5.0, 50.0, -0.3, true);
        limiter.set_enabled(true);
        for _ in 0..10000 {
            let (l, r) = limiter.process(2.0, 2.0);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_limiter_disabled_passthrough() {
        let mut limiter = LookaheadLimiter::new(44100.0);
        limiter.set_enabled(false);
        let (l, r) = limiter.process(0.5, 0.5);
        assert!((l - 0.5).abs() < 1e-5);
        assert!((r - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_limiter_reset() {
        let mut limiter = LookaheadLimiter::new(44100.0);
        limiter.set_ceiling_db(-1.0);
        for _ in 0..100 {
            limiter.process(1.0, 1.0);
        }
        limiter.reset();
        assert!((limiter.current_gain() - 1.0).abs() < 1e-5);
    }
}

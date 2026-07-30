use super::biquad::{BiquadCoeffs, BiquadState};

struct BandCompressor {
    threshold: f32, // linear
    threshold_db_cached: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    makeup_gain: f32, // linear
    envelope: f32,
}

impl BandCompressor {
    fn new(
        sample_rate: f32,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
    ) -> Self {
        let sample_rate = sample_rate.max(1.0);
        let attack_ms = attack_ms.max(0.0001);
        let release_ms = release_ms.max(0.0001);
        let ratio = ratio.clamp(1.0, 100.0);
        Self {
            threshold: 10.0_f32.powf(threshold_db / 20.0),
            threshold_db_cached: threshold_db.max(-100.0),
            ratio,
            attack_coeff: (-1.0 / (attack_ms * 0.001 * sample_rate)).exp(),
            release_coeff: (-1.0 / (release_ms * 0.001 * sample_rate)).exp(),
            makeup_gain: 10.0_f32.powf(makeup_db / 20.0),
            envelope: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        let abs_sample = sample.abs();

        // Simple envelope follower
        let coeff = if abs_sample > self.envelope { self.attack_coeff } else { self.release_coeff };
        self.envelope = abs_sample + coeff * (self.envelope - abs_sample);

        // Prevent denormals
        if self.envelope < 1e-6 {
            self.envelope = 0.0;
        }

        if self.envelope <= 0.0 || self.envelope <= self.threshold {
            return sample * self.makeup_gain;
        }

        // Calculate gain reduction in dB
        let env_db = 20.0 * self.envelope.log10().max(-100.0);
        let thresh_db = self.threshold_db_cached;
        let overshoot = env_db - thresh_db;
        let ratio = self.ratio.max(1.0);
        let reduced_overshoot = overshoot / ratio;
        let gain_reduction_db = overshoot - reduced_overshoot;
        let gain = 10.0_f32.powf(-gain_reduction_db / 20.0);

        sample * gain * self.makeup_gain
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

/// Linkwitz-Riley 4th order crossover (cascaded 2nd order Butterworth)
struct CrossoverFilter {
    lp1: BiquadState,
    lp2: BiquadState,
    hp1: BiquadState,
    hp2: BiquadState,
    lp_coeffs: BiquadCoeffs,
    hp_coeffs: BiquadCoeffs,
}

impl CrossoverFilter {
    fn new(sample_rate: f32, freq: f32) -> Self {
        Self {
            lp1: BiquadState::default(),
            lp2: BiquadState::default(),
            hp1: BiquadState::default(),
            hp2: BiquadState::default(),
            lp_coeffs: BiquadCoeffs::lowpass(sample_rate, freq, 0.707),
            hp_coeffs: BiquadCoeffs::highpass(sample_rate, freq, 0.707),
        }
    }

    #[inline]
    fn process(&mut self, sample: f32) -> (f32, f32) {
        let mut low = self.lp1.process(sample, &self.lp_coeffs);
        low = self.lp2.process(low, &self.lp_coeffs);

        let mut high = self.hp1.process(sample, &self.hp_coeffs);
        high = self.hp2.process(high, &self.hp_coeffs);

        (low, high)
    }

    fn reset(&mut self) {
        self.lp1.reset();
        self.lp2.reset();
        self.hp1.reset();
        self.hp2.reset();
    }
}

pub struct MultibandCompressor {
    enabled: bool,
    #[allow(dead_code)]
    sample_rate: f32,

    // Crossovers
    xover_low_mid_l: CrossoverFilter,
    xover_low_mid_r: CrossoverFilter,
    xover_mid_high_l: CrossoverFilter,
    xover_mid_high_r: CrossoverFilter,

    // Compressors (Stereo: L/R processed with same settings, but separate envelope state)
    comp_low_l: BandCompressor,
    comp_low_r: BandCompressor,
    comp_mid_l: BandCompressor,
    comp_mid_r: BandCompressor,
    comp_high_l: BandCompressor,
    comp_high_r: BandCompressor,
}

impl MultibandCompressor {
    pub fn new(sample_rate: f32) -> Self {
        let freq_low_mid = 250.0;
        let freq_mid_high = 4000.0;

        Self {
            enabled: false,
            sample_rate,

            xover_low_mid_l: CrossoverFilter::new(sample_rate, freq_low_mid),
            xover_low_mid_r: CrossoverFilter::new(sample_rate, freq_low_mid),
            xover_mid_high_l: CrossoverFilter::new(sample_rate, freq_mid_high),
            xover_mid_high_r: CrossoverFilter::new(sample_rate, freq_mid_high),

            // Lows: Thump catching
            comp_low_l: BandCompressor::new(sample_rate, -10.0, 4.0, 10.0, 100.0, 2.0),
            comp_low_r: BandCompressor::new(sample_rate, -10.0, 4.0, 10.0, 100.0, 2.0),
            // Mids: Gentle glue
            comp_mid_l: BandCompressor::new(sample_rate, -15.0, 2.0, 30.0, 200.0, 1.0),
            comp_mid_r: BandCompressor::new(sample_rate, -15.0, 2.0, 30.0, 200.0, 1.0),
            // Highs: Peak taming (de-esser style)
            comp_high_l: BandCompressor::new(sample_rate, -12.0, 3.0, 5.0, 50.0, 0.0),
            comp_high_r: BandCompressor::new(sample_rate, -12.0, 3.0, 5.0, 50.0, 0.0),
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            if !enabled {
                self.reset();
            }
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        // Preserve user-configured band parameters across sample rate changes.

        let enabled = self.enabled;
        let old_rate = self.sample_rate;

        // Snapshot the current band parameters so we can restore them after
        // rebuilding the crossover filters (whose coefficients depend on the
        // new sample rate).
        let snapshot = |comp: &BandCompressor, sr: f32| -> (f32, f32, f32, f32, f32) {
            // Recover the dB/linear values from the cached fields. The
            // BandCompressor stores threshold_db_cached and makeup_gain
            // (linear); we convert makeup back to dB for a clean round-trip.
            (
                comp.threshold_db_cached,
                comp.ratio,
                // attack/release coefficients are sample-rate-dependent, so
                // we invert the coefficient formula to recover the time in ms:
                //   coeff = exp(-1 / (secs * sr))
                //   secs  = -1 / (sr * ln(coeff))
                //   ms    = secs * 1000
                // For coeff == 0 (instant attack) or coeff == 1 (no smoothing),
                // fall back to safe defaults.
                if comp.attack_coeff > 0.0 && comp.attack_coeff < 1.0 && sr > 0.0 {
                    (-1.0 / (sr * comp.attack_coeff.ln())) * 1000.0
                } else {
                    0.0001
                },
                if comp.release_coeff > 0.0 && comp.release_coeff < 1.0 && sr > 0.0 {
                    (-1.0 / (sr * comp.release_coeff.ln())) * 1000.0
                } else {
                    0.001
                },
                20.0 * comp.makeup_gain.log10(),
            )
        };

        let low_params = snapshot(&self.comp_low_l, old_rate);
        let mid_params = snapshot(&self.comp_mid_l, old_rate);
        let high_params = snapshot(&self.comp_high_l, old_rate);

        // Rebuild at the new sample rate with default band params, then
        // re-apply the snapshotted user params.
        *self = Self::new(sample_rate);
        self.enabled = enabled;
        self.set_band_params(
            0,
            low_params.0,
            low_params.1,
            low_params.2,
            low_params.3,
            low_params.4,
        );
        self.set_band_params(
            1,
            mid_params.0,
            mid_params.1,
            mid_params.2,
            mid_params.3,
            mid_params.4,
        );
        self.set_band_params(
            2,
            high_params.0,
            high_params.1,
            high_params.2,
            high_params.3,
            high_params.4,
        );
    }

    pub fn set_band_params(
        &mut self,
        band: usize,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_gain_db: f32,
    ) {
        let (comp_l, comp_r) = match band {
            0 => (&mut self.comp_low_l, &mut self.comp_low_r),
            1 => (&mut self.comp_mid_l, &mut self.comp_mid_r),
            2 => (&mut self.comp_high_l, &mut self.comp_high_r),
            _ => return,
        };

        let mut comp_new_l = BandCompressor::new(
            self.sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_gain_db,
        );
        let mut comp_new_r = BandCompressor::new(
            self.sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_gain_db,
        );

        comp_new_l.envelope = comp_l.envelope;
        comp_new_r.envelope = comp_r.envelope;

        *comp_l = comp_new_l;
        *comp_r = comp_new_r;
    }

    #[inline]
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if !self.enabled {
            return (left, right);
        }

        // Split Left
        let (l_low, l_mid_high) = self.xover_low_mid_l.process(left);
        let (l_mid, l_high) = self.xover_mid_high_l.process(l_mid_high);

        // Split Right
        let (r_low, r_mid_high) = self.xover_low_mid_r.process(right);
        let (r_mid, r_high) = self.xover_mid_high_r.process(r_mid_high);

        // Compress
        let l_low_c = self.comp_low_l.process(l_low);
        let r_low_c = self.comp_low_r.process(r_low);
        let l_mid_c = self.comp_mid_l.process(l_mid);
        let r_mid_c = self.comp_mid_r.process(r_mid);
        let l_high_c = self.comp_high_l.process(l_high);
        let r_high_c = self.comp_high_r.process(r_high);

        // Sum back
        (l_low_c + l_mid_c + l_high_c, r_low_c + r_mid_c + r_high_c)
    }

    pub fn reset(&mut self) {
        self.xover_low_mid_l.reset();
        self.xover_low_mid_r.reset();
        self.xover_mid_high_l.reset();
        self.xover_mid_high_r.reset();

        self.comp_low_l.reset();
        self.comp_low_r.reset();
        self.comp_mid_l.reset();
        self.comp_mid_r.reset();
        self.comp_high_l.reset();
        self.comp_high_r.reset();
    }
}

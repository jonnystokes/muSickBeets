// ============================================================================
// FILTERS.RS - Frequency Filtering Effects
// ============================================================================
//
// Biquad-based filters for frequency shaping:
// - Low-pass: Remove high frequencies
// - High-pass: Remove low frequencies
// - Band-pass: Pass only a frequency range
// - Notch: Remove a specific frequency
// - Peak/Bell: Boost or cut around a frequency
// - Shelving: Boost or cut above/below a frequency
//
// All filters use the standard biquad (second-order IIR) topology
// with coefficient calculation based on Audio EQ Cookbook.
// ============================================================================

use super::{Effect, StereoSample, EffectContext, PI};
use super::state::{FilterParams, FilterType};

// ============================================================================
// BIQUAD FILTER
// ============================================================================
//
// General-purpose biquad filter that can implement various filter types.
// Transfer function: H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
// ============================================================================

/// Biquad filter coefficients
#[derive(Clone, Debug, Default)]
pub struct BiquadCoefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl BiquadCoefficients {
    /// Calculate low-pass filter coefficients
    pub fn low_pass(cutoff_hz: f32, q: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate high-pass filter coefficients
    pub fn high_pass(cutoff_hz: f32, q: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate band-pass filter coefficients (constant skirt gain)
    pub fn band_pass(center_hz: f32, q: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * center_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate notch filter coefficients
    pub fn notch(center_hz: f32, q: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * center_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate peaking EQ filter coefficients
    pub fn peak(center_hz: f32, q: f32, gain_db: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * center_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate low shelf filter coefficients
    pub fn low_shelf(cutoff_hz: f32, gain_db: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * ((a + 1.0/a) * (1.0/0.9 - 1.0) + 2.0).sqrt();

        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate high shelf filter coefficients
    pub fn high_shelf(cutoff_hz: f32, gain_db: f32, sample_rate: u32) -> Self {
        let w0 = 2.0 * PI * cutoff_hz / sample_rate as f32;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0_f32.powf(gain_db / 40.0);
        let alpha = sin_w0 / 2.0 * ((a + 1.0/a) * (1.0/0.9 - 1.0) + 2.0).sqrt();

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

/// Biquad filter state (delay line)
#[derive(Clone, Debug, Default)]
pub struct BiquadState {
    z1: f32,  // z^-1 delay
    z2: f32,  // z^-2 delay
}

impl BiquadState {
    pub fn new() -> Self {
        Self { z1: 0.0, z2: 0.0 }
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    /// Process one sample through the filter (transposed direct form II)
    pub fn process(&mut self, input: f32, coef: &BiquadCoefficients) -> f32 {
        let output = coef.b0 * input + self.z1;
        self.z1 = coef.b1 * input - coef.a1 * output + self.z2;
        self.z2 = coef.b2 * input - coef.a2 * output;
        output
    }
}

// ============================================================================
// FILTER EFFECT
// ============================================================================
//
// User-facing filter effect that wraps the biquad implementation.
// ============================================================================

/// Filter effect
pub struct FilterEffect {
    pub params: FilterParams,
    pub active: bool,
    pub mix: f32,
    coefficients: BiquadCoefficients,
    state_left: BiquadState,
    state_right: BiquadState,
    last_cutoff: f32,
    last_resonance: f32,
    last_filter_type: FilterType,
    sample_rate: u32,
}

impl FilterEffect {
    pub fn new(sample_rate: u32) -> Self {
        let mut effect = Self {
            params: FilterParams::default(),
            active: true,
            mix: 1.0,
            coefficients: BiquadCoefficients::default(),
            state_left: BiquadState::new(),
            state_right: BiquadState::new(),
            last_cutoff: 0.0,
            last_resonance: 0.0,
            last_filter_type: FilterType::LowPass,
            sample_rate,
        };
        effect.update_coefficients();
        effect
    }

    fn update_coefficients(&mut self) {
        // Only recalculate if parameters changed
        if self.params.cutoff_hz != self.last_cutoff
            || self.params.resonance != self.last_resonance
            || self.params.filter_type != self.last_filter_type
        {
            self.coefficients = match self.params.filter_type {
                FilterType::LowPass => BiquadCoefficients::low_pass(
                    self.params.cutoff_hz, self.params.resonance, self.sample_rate),
                FilterType::HighPass => BiquadCoefficients::high_pass(
                    self.params.cutoff_hz, self.params.resonance, self.sample_rate),
                FilterType::BandPass => BiquadCoefficients::band_pass(
                    self.params.cutoff_hz, self.params.resonance, self.sample_rate),
                FilterType::Notch => BiquadCoefficients::notch(
                    self.params.cutoff_hz, self.params.resonance, self.sample_rate),
                FilterType::Peak => BiquadCoefficients::peak(
                    self.params.cutoff_hz, self.params.resonance, 0.0, self.sample_rate),
                FilterType::LowShelf => BiquadCoefficients::low_shelf(
                    self.params.cutoff_hz, 0.0, self.sample_rate),
                FilterType::HighShelf => BiquadCoefficients::high_shelf(
                    self.params.cutoff_hz, 0.0, self.sample_rate),
            };

            self.last_cutoff = self.params.cutoff_hz;
            self.last_resonance = self.params.resonance;
            self.last_filter_type = self.params.filter_type;
        }
    }
}

impl Effect for FilterEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        self.update_coefficients();

        // Apply optional drive (saturation before filter)
        let driven_l = if self.params.drive > 1.0 {
            super::soft_clip(input.left * self.params.drive) / self.params.drive
        } else {
            input.left
        };
        let driven_r = if self.params.drive > 1.0 {
            super::soft_clip(input.right * self.params.drive) / self.params.drive
        } else {
            input.right
        };

        let filtered_l = self.state_left.process(driven_l, &self.coefficients);
        let filtered_r = self.state_right.process(driven_r, &self.coefficients);

        if self.mix >= 1.0 {
            StereoSample { left: filtered_l, right: filtered_r }
        } else {
            StereoSample {
                left: input.left * (1.0 - self.mix) + filtered_l * self.mix,
                right: input.right * (1.0 - self.mix) + filtered_r * self.mix,
            }
        }
    }

    fn reset(&mut self) {
        self.state_left.reset();
        self.state_right.reset();
    }

    fn name(&self) -> &'static str {
        match self.params.filter_type {
            FilterType::LowPass => "lowpass",
            FilterType::HighPass => "highpass",
            FilterType::BandPass => "bandpass",
            FilterType::Notch => "notch",
            FilterType::Peak => "peak",
            FilterType::LowShelf => "lowshelf",
            FilterType::HighShelf => "highshelf",
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// CONVENIENCE FILTER CONSTRUCTORS
// ============================================================================

/// Low-pass filter effect
pub struct LowPassFilter {
    filter: FilterEffect,
}

impl LowPassFilter {
    pub fn new(sample_rate: u32, cutoff_hz: f32, resonance: f32) -> Self {
        let mut filter = FilterEffect::new(sample_rate);
        filter.params.filter_type = FilterType::LowPass;
        filter.params.cutoff_hz = cutoff_hz;
        filter.params.resonance = resonance;
        filter.update_coefficients();
        Self { filter }
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32) {
        self.filter.params.cutoff_hz = cutoff_hz;
    }

    pub fn set_resonance(&mut self, resonance: f32) {
        self.filter.params.resonance = resonance;
    }
}

impl Effect for LowPassFilter {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        self.filter.process(input, ctx)
    }

    fn reset(&mut self) {
        self.filter.reset();
    }

    fn name(&self) -> &'static str { "lowpass" }
    fn is_active(&self) -> bool { self.filter.is_active() }
    fn set_mix(&mut self, mix: f32) { self.filter.set_mix(mix); }
    fn get_mix(&self) -> f32 { self.filter.get_mix() }
}

/// High-pass filter effect
pub struct HighPassFilter {
    filter: FilterEffect,
}

impl HighPassFilter {
    pub fn new(sample_rate: u32, cutoff_hz: f32, resonance: f32) -> Self {
        let mut filter = FilterEffect::new(sample_rate);
        filter.params.filter_type = FilterType::HighPass;
        filter.params.cutoff_hz = cutoff_hz;
        filter.params.resonance = resonance;
        filter.update_coefficients();
        Self { filter }
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32) {
        self.filter.params.cutoff_hz = cutoff_hz;
    }

    pub fn set_resonance(&mut self, resonance: f32) {
        self.filter.params.resonance = resonance;
    }
}

impl Effect for HighPassFilter {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        self.filter.process(input, ctx)
    }

    fn reset(&mut self) {
        self.filter.reset();
    }

    fn name(&self) -> &'static str { "highpass" }
    fn is_active(&self) -> bool { self.filter.is_active() }
    fn set_mix(&mut self, mix: f32) { self.filter.set_mix(mix); }
    fn get_mix(&self) -> f32 { self.filter.get_mix() }
}

// ============================================================================
// DYNAMICS.RS - Dynamic Range and Distortion Effects
// ============================================================================
//
// Effects that modify the dynamic range or add harmonic content:
// - Distortion: Adds harmonics through clipping/saturation
// - Bitcrush: Lo-fi bit depth and sample rate reduction
// - Compressor: Dynamic range compression
// - Limiter: Hard limiting to prevent clipping
// - Gate: Noise gate
//
// ============================================================================

use super::{Effect, StereoSample, EffectContext, soft_clip, hard_clip, db_to_linear, linear_to_db, lerp};
use super::state::{DistortionParams, DistortionType, BitcrushParams, CompressorParams};
use std::f32::consts::PI;

// ============================================================================
// DISTORTION EFFECT
// ============================================================================
//
// Various types of distortion/saturation for adding harmonics and grit.
// ============================================================================

/// Distortion effect with multiple algorithms
pub struct DistortionEffect {
    pub params: DistortionParams,
    pub active: bool,
}

impl DistortionEffect {
    pub fn new() -> Self {
        Self {
            params: DistortionParams::default(),
            active: true,
        }
    }

    fn apply_distortion(&self, sample: f32) -> f32 {
        let driven = sample * self.params.drive;

        match self.params.distortion_type {
            DistortionType::SoftClip => {
                // Smooth tanh-style saturation
                soft_clip(driven)
            },
            DistortionType::HardClip => {
                // Digital hard clipping
                hard_clip(driven, -1.0, 1.0)
            },
            DistortionType::Foldback => {
                // Wave folding - folds signal back when it exceeds threshold
                let mut x = driven;
                while x.abs() > 1.0 {
                    if x > 1.0 {
                        x = 2.0 - x;
                    } else if x < -1.0 {
                        x = -2.0 - x;
                    }
                }
                x
            },
            DistortionType::Tube => {
                // Asymmetric tube-like saturation
                if driven >= 0.0 {
                    1.0 - (-driven * 3.0).exp()
                } else {
                    -1.0 + (driven * 2.0).exp()
                }
            },
            DistortionType::Fuzz => {
                // Square-ish fuzz with smooth transitions
                let x = driven.clamp(-1.0, 1.0);
                x.signum() * (1.0 - (1.0 - x.abs()).powi(3))
            },
        }
    }
}

impl Default for DistortionEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for DistortionEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.drive <= 1.0 {
            return input;
        }

        let dist_l = self.apply_distortion(input.left) * self.params.output_gain;
        let dist_r = self.apply_distortion(input.right) * self.params.output_gain;

        // Simple tone control (low-pass blend)
        let tone = self.params.tone;
        let filtered_l = dist_l * tone + input.left * (1.0 - tone) * self.params.output_gain;
        let filtered_r = dist_r * tone + input.right * (1.0 - tone) * self.params.output_gain;

        // Mix
        if self.params.mix >= 1.0 {
            StereoSample { left: filtered_l, right: filtered_r }
        } else {
            StereoSample {
                left: input.left * (1.0 - self.params.mix) + filtered_l * self.params.mix,
                right: input.right * (1.0 - self.params.mix) + filtered_r * self.params.mix,
            }
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn name(&self) -> &'static str {
        "distortion"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.drive > 1.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// BITCRUSH EFFECT
// ============================================================================
//
// Lo-fi effect that reduces bit depth and/or sample rate.
// Includes wet/dry mix and optional dithering.
// ============================================================================

/// Bitcrush effect
pub struct BitcrushEffect {
    pub params: BitcrushParams,
    pub active: bool,
    hold_left: f32,
    hold_right: f32,
    counter: f32,
    random_state: u32,
}

impl BitcrushEffect {
    pub fn new() -> Self {
        Self {
            params: BitcrushParams::default(),
            active: true,
            hold_left: 0.0,
            hold_right: 0.0,
            counter: 0.0,
            random_state: 12345,
        }
    }

    fn quantize(&mut self, sample: f32) -> f32 {
        let bits = self.params.bits.clamp(1, 16) as f32;
        let levels = 2.0_f32.powf(bits);

        // Optional dithering
        let dither = if self.params.dither {
            self.random_state = self.random_state.wrapping_mul(1103515245).wrapping_add(12345);
            let r = ((self.random_state >> 16) & 0x7FFF) as f32 / 32768.0 - 0.5;
            r / levels
        } else {
            0.0
        };

        // Quantize
        ((sample + dither) * levels / 2.0).round() * 2.0 / levels
    }
}

impl Default for BitcrushEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for BitcrushEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active || (self.params.bits >= 16 && self.params.sample_rate_reduction <= 1.0) {
            return input;
        }

        // Sample rate reduction (sample & hold)
        self.counter += 1.0;
        if self.counter >= self.params.sample_rate_reduction {
            self.counter -= self.params.sample_rate_reduction;
            self.hold_left = self.quantize(input.left);
            self.hold_right = self.quantize(input.right);
        }

        // Mix
        if self.params.mix >= 1.0 {
            StereoSample { left: self.hold_left, right: self.hold_right }
        } else {
            StereoSample {
                left: input.left * (1.0 - self.params.mix) + self.hold_left * self.params.mix,
                right: input.right * (1.0 - self.params.mix) + self.hold_right * self.params.mix,
            }
        }
    }

    fn reset(&mut self) {
        self.hold_left = 0.0;
        self.hold_right = 0.0;
        self.counter = 0.0;
    }

    fn name(&self) -> &'static str {
        "bitcrush"
    }

    fn is_active(&self) -> bool {
        self.active && (self.params.bits < 16 || self.params.sample_rate_reduction > 1.0)
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// COMPRESSOR EFFECT
// ============================================================================
//
// Dynamic range compressor with attack/release, ratio, and soft knee.
// ============================================================================

/// Compressor effect
pub struct CompressorEffect {
    pub params: CompressorParams,
    pub active: bool,
    envelope: f32,
    gain_reduction_db: f32,
}

impl CompressorEffect {
    pub fn new() -> Self {
        Self {
            params: CompressorParams::default(),
            active: true,
            envelope: 0.0,
            gain_reduction_db: 0.0,
        }
    }

    fn compute_gain(&self, input_db: f32) -> f32 {
        let threshold = self.params.threshold_db;
        let ratio = self.params.ratio.max(1.0);
        let knee = self.params.knee_db;

        // Soft knee implementation
        let knee_start = threshold - knee / 2.0;
        let knee_end = threshold + knee / 2.0;

        if input_db <= knee_start {
            // Below knee - no compression
            0.0
        } else if input_db >= knee_end {
            // Above knee - full compression
            (threshold - input_db) + (input_db - threshold) / ratio
        } else {
            // In knee - smooth transition
            let knee_factor = (input_db - knee_start) / knee;
            let compressed = (threshold - input_db) + (input_db - threshold) / ratio;
            compressed * knee_factor * knee_factor / 2.0
        }
    }
}

impl Default for CompressorEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for CompressorEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        // Get input level (use max of L/R for stereo linking)
        let input_level = input.left.abs().max(input.right.abs());
        let input_db = linear_to_db(input_level);

        // Envelope follower with attack/release
        let attack_coef = (-1.0 / (self.params.attack_ms / 1000.0 * ctx.sample_rate as f32)).exp();
        let release_coef = (-1.0 / (self.params.release_ms / 1000.0 * ctx.sample_rate as f32)).exp();

        let coef = if input_db > self.envelope { attack_coef } else { release_coef };
        self.envelope = input_db + coef * (self.envelope - input_db);

        // Compute gain reduction
        self.gain_reduction_db = self.compute_gain(self.envelope);

        // Apply gain (including makeup gain)
        let total_gain_db = self.gain_reduction_db + self.params.makeup_gain_db;
        let gain = db_to_linear(total_gain_db);

        StereoSample {
            left: input.left * gain,
            right: input.right * gain,
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gain_reduction_db = 0.0;
    }

    fn name(&self) -> &'static str {
        "compressor"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_mix(&mut self, _mix: f32) {
        // Compressor typically doesn't use wet/dry mix
    }

    fn get_mix(&self) -> f32 {
        1.0
    }
}

// ============================================================================
// LIMITER EFFECT
// ============================================================================
//
// Hard limiter to prevent clipping. Uses lookahead for clean limiting.
// ============================================================================

/// Limiter effect
pub struct LimiterEffect {
    pub ceiling_db: f32,
    pub release_ms: f32,
    pub active: bool,
    envelope: f32,
}

impl LimiterEffect {
    pub fn new() -> Self {
        Self {
            ceiling_db: -0.3,
            release_ms: 100.0,
            active: true,
            envelope: 0.0,
        }
    }
}

impl Default for LimiterEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for LimiterEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        let ceiling = db_to_linear(self.ceiling_db);

        // Get peak level
        let peak = input.left.abs().max(input.right.abs());

        // Fast attack, slower release envelope
        let release_coef = (-1.0 / (self.release_ms / 1000.0 * ctx.sample_rate as f32)).exp();

        if peak > self.envelope {
            self.envelope = peak; // Instant attack
        } else {
            self.envelope = peak + release_coef * (self.envelope - peak);
        }

        // Calculate gain reduction
        let gain = if self.envelope > ceiling {
            ceiling / self.envelope
        } else {
            1.0
        };

        StereoSample {
            left: input.left * gain,
            right: input.right * gain,
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
    }

    fn name(&self) -> &'static str {
        "limiter"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_mix(&mut self, _mix: f32) {}
    fn get_mix(&self) -> f32 { 1.0 }
}

// ============================================================================
// NOISE GATE EFFECT
// ============================================================================
//
// Attenuates signal below a threshold to reduce noise.
// ============================================================================

/// Noise gate effect
pub struct GateEffect {
    pub threshold_db: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub range_db: f32,  // How much to attenuate when closed
    pub active: bool,
    envelope: f32,
    gate_gain: f32,
}

impl GateEffect {
    pub fn new() -> Self {
        Self {
            threshold_db: -40.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            range_db: -80.0,
            active: true,
            envelope: 0.0,
            gate_gain: 0.0,
        }
    }
}

impl Default for GateEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for GateEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        // Get input level
        let input_level = input.left.abs().max(input.right.abs());
        let input_db = linear_to_db(input_level);

        // Determine target gate state
        let target_gain = if input_db > self.threshold_db {
            1.0
        } else {
            db_to_linear(self.range_db)
        };

        // Smooth gate with attack/release
        let attack_coef = 1.0 - (-1.0 / (self.attack_ms / 1000.0 * ctx.sample_rate as f32)).exp();
        let release_coef = 1.0 - (-1.0 / (self.release_ms / 1000.0 * ctx.sample_rate as f32)).exp();

        let coef = if target_gain > self.gate_gain { attack_coef } else { release_coef };
        self.gate_gain += coef * (target_gain - self.gate_gain);

        StereoSample {
            left: input.left * self.gate_gain,
            right: input.right * self.gate_gain,
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gate_gain = 0.0;
    }

    fn name(&self) -> &'static str {
        "gate"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_mix(&mut self, _mix: f32) {}
    fn get_mix(&self) -> f32 { 1.0 }
}

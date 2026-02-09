// ============================================================================
// CORE.RS - Basic Audio Effects (Amplitude, Pan)
// ============================================================================
//
// These are fundamental effects that form the basis of audio processing.
// They work identically on channels and master bus.
//
// ============================================================================

use super::{Effect, StereoSample, EffectContext, pan_coefficients, db_to_linear};
use super::state::{AmplitudeParams, PanParams};

// ============================================================================
// AMPLITUDE EFFECT
// ============================================================================
//
// Simple gain/volume control. Can use linear gain or decibels.
// ============================================================================

/// Amplitude (gain/volume) effect
pub struct AmplitudeEffect {
    pub params: AmplitudeParams,
    pub mix: f32,
    pub active: bool,
}

impl AmplitudeEffect {
    pub fn new() -> Self {
        Self {
            params: AmplitudeParams::default(),
            mix: 1.0,
            active: true,
        }
    }

    pub fn with_gain(mut self, gain: f32) -> Self {
        self.params.gain = gain;
        self
    }

    pub fn with_db(mut self, db: f32) -> Self {
        self.params.gain_db = db;
        self.params.gain = db_to_linear(db);
        self
    }

    /// Get the effective gain value
    pub fn effective_gain(&self) -> f32 {
        self.params.gain
    }
}

impl Default for AmplitudeEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for AmplitudeEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        let gain = self.effective_gain();

        StereoSample {
            left: input.left * gain,
            right: input.right * gain,
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn name(&self) -> &'static str {
        "amplitude"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.gain != 1.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// PAN EFFECT
// ============================================================================
//
// Stereo panning using equal-power pan law.
// -1.0 = full left, 0.0 = center, 1.0 = full right
// ============================================================================

/// Stereo panning effect
pub struct PanEffect {
    pub params: PanParams,
    pub mix: f32,
    pub active: bool,
}

impl PanEffect {
    pub fn new() -> Self {
        Self {
            params: PanParams::default(),
            mix: 1.0,
            active: true,
        }
    }

    pub fn with_position(mut self, position: f32) -> Self {
        self.params.position = position.clamp(-1.0, 1.0);
        self
    }
}

impl Default for PanEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for PanEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.position == 0.0 {
            return input;
        }

        let (left_gain, right_gain) = pan_coefficients(self.params.position);

        // For mono source, apply pan directly
        // For stereo source, blend based on pan position
        let mono = input.to_mono();

        if self.mix >= 1.0 {
            StereoSample {
                left: mono * left_gain,
                right: mono * right_gain,
            }
        } else {
            // Blend between original stereo and panned mono
            let panned_left = mono * left_gain;
            let panned_right = mono * right_gain;

            StereoSample {
                left: input.left * (1.0 - self.mix) + panned_left * self.mix,
                right: input.right * (1.0 - self.mix) + panned_right * self.mix,
            }
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn name(&self) -> &'static str {
        "pan"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.position != 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// STEREO WIDTH EFFECT
// ============================================================================
//
// Adjusts the stereo width of the signal.
// 0.0 = mono, 1.0 = normal stereo, 2.0 = extra wide
// ============================================================================

/// Stereo width adjustment effect
pub struct StereoWidthEffect {
    pub width: f32,
    pub mix: f32,
    pub active: bool,
}

impl StereoWidthEffect {
    pub fn new() -> Self {
        Self {
            width: 1.0,
            mix: 1.0,
            active: true,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width.max(0.0);
        self
    }
}

impl Default for StereoWidthEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for StereoWidthEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active || self.width == 1.0 {
            return input;
        }

        // Mid-side processing
        let mid = (input.left + input.right) * 0.5;
        let side = (input.left - input.right) * 0.5;

        // Adjust side level based on width
        let adjusted_side = side * self.width;

        // Convert back to left/right
        let processed = StereoSample {
            left: mid + adjusted_side,
            right: mid - adjusted_side,
        };

        if self.mix >= 1.0 {
            processed
        } else {
            StereoSample {
                left: input.left * (1.0 - self.mix) + processed.left * self.mix,
                right: input.right * (1.0 - self.mix) + processed.right * self.mix,
            }
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }

    fn name(&self) -> &'static str {
        "width"
    }

    fn is_active(&self) -> bool {
        self.active && self.width != 1.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// DC OFFSET FILTER
// ============================================================================
//
// Removes DC offset from the signal. Essential for preventing speaker damage
// and keeping effects working correctly.
// ============================================================================

/// DC offset removal filter
pub struct DcFilterEffect {
    pub coefficient: f32,
    prev_input_l: f32,
    prev_input_r: f32,
    prev_output_l: f32,
    prev_output_r: f32,
    pub active: bool,
}

impl DcFilterEffect {
    pub fn new() -> Self {
        Self {
            coefficient: 0.995, // High-pass at ~5Hz for 48kHz sample rate
            prev_input_l: 0.0,
            prev_input_r: 0.0,
            prev_output_l: 0.0,
            prev_output_r: 0.0,
            active: true,
        }
    }
}

impl Default for DcFilterEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for DcFilterEffect {
    fn process(&mut self, input: StereoSample, _ctx: &EffectContext) -> StereoSample {
        if !self.active {
            return input;
        }

        // Simple one-pole high-pass filter
        // y[n] = coef * (y[n-1] + x[n] - x[n-1])
        let out_l = self.coefficient * (self.prev_output_l + input.left - self.prev_input_l);
        let out_r = self.coefficient * (self.prev_output_r + input.right - self.prev_input_r);

        self.prev_input_l = input.left;
        self.prev_input_r = input.right;
        self.prev_output_l = out_l;
        self.prev_output_r = out_r;

        StereoSample {
            left: out_l,
            right: out_r,
        }
    }

    fn reset(&mut self) {
        self.prev_input_l = 0.0;
        self.prev_input_r = 0.0;
        self.prev_output_l = 0.0;
        self.prev_output_r = 0.0;
    }

    fn name(&self) -> &'static str {
        "dc_filter"
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_mix(&mut self, _mix: f32) {
        // DC filter doesn't use mix
    }

    fn get_mix(&self) -> f32 {
        1.0
    }
}

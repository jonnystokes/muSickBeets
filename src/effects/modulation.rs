// ============================================================================
// MODULATION.RS - Time-Based Modulation Effects
// ============================================================================
//
// Modulation effects use LFOs (Low Frequency Oscillators) to vary parameters
// over time, creating movement and depth in the sound.
//
// Effects in this module:
// - Vibrato: Pitch modulation
// - Tremolo: Amplitude modulation
// - Chorus: Delayed/modulated copies of the signal
// - Phaser: All-pass filter sweeps
// - Flanger: Short modulated delay with feedback
// - Auto-pan: Automatic stereo panning
//
// ============================================================================

use super::{Effect, StereoSample, EffectContext, lerp, TWO_PI, PI};
use super::state::{
    VibratoParams, TremoloParams, ChorusParams, PhaserParams, FlangerParams,
    LfoWaveform, EffectBuffer, StereoBuffer,
};

// ============================================================================
// LFO UTILITIES
// ============================================================================

/// Generate LFO value for given phase and waveform
fn lfo_value(phase: f32, waveform: LfoWaveform, random_state: &mut u32) -> f32 {
    match waveform {
        LfoWaveform::Sine => (phase * TWO_PI).sin(),
        LfoWaveform::Triangle => {
            let p = phase * 4.0;
            if p < 1.0 {
                p
            } else if p < 3.0 {
                2.0 - p
            } else {
                p - 4.0
            }
        },
        LfoWaveform::Square => {
            if phase < 0.5 { 1.0 } else { -1.0 }
        },
        LfoWaveform::Saw => {
            phase * 2.0 - 1.0
        },
        LfoWaveform::Random => {
            // Only update on phase wrap (simple S&H)
            if phase < 0.01 {
                *random_state = random_state.wrapping_mul(1103515245).wrapping_add(12345);
                ((*random_state >> 16) & 0x7FFF) as f32 / 16383.5 - 1.0
            } else {
                // Return last value (handled by caller storing it)
                0.0 // Placeholder - caller should maintain held value
            }
        }
    }
}

/// Advance LFO phase
#[inline]
fn advance_phase(phase: &mut f32, rate_hz: f32, sample_rate: u32) {
    *phase += rate_hz / sample_rate as f32;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
}

// ============================================================================
// VIBRATO EFFECT
// ============================================================================
//
// Vibrato modulates the pitch of the signal using an LFO.
// For channel use: Modulates the oscillator frequency directly
// For master use: Uses pitch shifting (requires pitch_shift module)
// ============================================================================

/// Vibrato effect (pitch modulation)
pub struct VibratoEffect {
    pub params: VibratoParams,
    pub mix: f32,
    pub active: bool,
    pub phase: f32,
    pub random_state: u32,
    pub random_hold: f32,
    /// For master bus: delay buffer for pitch shifting
    delay_buffer: EffectBuffer,
    read_offset: f32,
}

impl VibratoEffect {
    pub fn new(sample_rate: u32) -> Self {
        // Buffer for ~50ms max pitch shift delay
        let buffer_size = (sample_rate as f32 * 0.05) as usize;
        Self {
            params: VibratoParams::default(),
            mix: 1.0,
            active: true,
            phase: 0.0,
            random_state: 54321,
            random_hold: 0.0,
            delay_buffer: EffectBuffer::new(buffer_size),
            read_offset: 0.0,
        }
    }

    /// Get the current frequency multiplier for direct oscillator modulation
    pub fn get_frequency_multiplier(&self) -> f32 {
        let total_semitones = self.params.depth_semitones + self.params.depth_cents / 100.0;
        let lfo = (self.phase * TWO_PI).sin(); // Simple sine for now
        2.0_f32.powf(lfo * total_semitones / 12.0)
    }

    /// Process for master bus (uses delay-based pitch shifting)
    fn process_master(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        // Simple delay-based vibrato for master
        // Write input to buffer
        let mono = input.to_mono();
        self.delay_buffer.write(mono);

        // Calculate modulated delay
        let total_semitones = self.params.depth_semitones + self.params.depth_cents / 100.0;
        let lfo = lfo_value(self.phase, self.params.waveform, &mut self.random_state);

        // Map pitch to delay modulation
        let delay_mod = lfo * total_semitones * 0.001 * ctx.sample_rate as f32;
        let base_delay = self.delay_buffer.size as f32 / 2.0;
        let delay = (base_delay + delay_mod).clamp(1.0, self.delay_buffer.size as f32 - 2.0);

        let delayed = self.delay_buffer.read_interpolated(delay);

        // Advance phase
        advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);

        // Mix
        if self.mix >= 1.0 {
            StereoSample::mono(delayed)
        } else {
            StereoSample {
                left: input.left * (1.0 - self.mix) + delayed * self.mix,
                right: input.right * (1.0 - self.mix) + delayed * self.mix,
            }
        }
    }
}

impl Effect for VibratoEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || (self.params.depth_semitones == 0.0 && self.params.depth_cents == 0.0) {
            return input;
        }

        // If we have input frequency info, caller should use get_frequency_multiplier()
        // Otherwise, use delay-based pitch shifting for master bus
        if ctx.input_frequency > 0.0 {
            // Channel mode - just advance phase, multiplier is read separately
            advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);
            input
        } else {
            self.process_master(input, ctx)
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.delay_buffer.clear();
        self.read_offset = 0.0;
    }

    fn name(&self) -> &'static str {
        "vibrato"
    }

    fn is_active(&self) -> bool {
        self.active && (self.params.depth_semitones != 0.0 || self.params.depth_cents != 0.0)
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// TREMOLO EFFECT
// ============================================================================
//
// Tremolo modulates the amplitude of the signal using an LFO.
// Works identically on channels and master bus.
// ============================================================================

/// Tremolo effect (amplitude modulation)
pub struct TremoloEffect {
    pub params: TremoloParams,
    pub mix: f32,
    pub active: bool,
    pub phase: f32,
    pub random_state: u32,
    pub random_hold: f32,
}

impl TremoloEffect {
    pub fn new() -> Self {
        Self {
            params: TremoloParams::default(),
            mix: 1.0,
            active: true,
            phase: 0.0,
            random_state: 98765,
            random_hold: 0.0,
        }
    }
}

impl Default for TremoloEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for TremoloEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.depth == 0.0 {
            return input;
        }

        // Generate LFO value (-1 to 1)
        let lfo = lfo_value(self.phase, self.params.waveform, &mut self.random_state);

        // Convert to gain modulation (centered around 1.0)
        // depth of 1.0 means full modulation from 0 to 2
        let gain = 1.0 + lfo * self.params.depth;

        // Advance phase
        advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);

        let processed = StereoSample {
            left: input.left * gain,
            right: input.right * gain,
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
        self.phase = 0.0;
    }

    fn name(&self) -> &'static str {
        "tremolo"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.depth > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// CHORUS EFFECT
// ============================================================================
//
// Chorus creates a thicker sound by mixing delayed and pitch-modulated
// copies of the signal with the original.
// ============================================================================

/// Chorus effect
pub struct ChorusEffect {
    pub params: ChorusParams,
    pub active: bool,
    pub phase: f32,
    buffer_left: EffectBuffer,
    buffer_right: EffectBuffer,
}

impl ChorusEffect {
    pub fn new(sample_rate: u32) -> Self {
        let buffer_size = (sample_rate as f32 * 0.1) as usize; // 100ms max
        Self {
            params: ChorusParams::default(),
            active: true,
            phase: 0.0,
            buffer_left: EffectBuffer::new(buffer_size),
            buffer_right: EffectBuffer::new(buffer_size),
        }
    }
}

impl Effect for ChorusEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.mix == 0.0 {
            return input;
        }

        // Write to buffers
        self.buffer_left.write(input.left);
        self.buffer_right.write(input.right);

        // Calculate modulated delay
        let base_delay_samples = 20.0 / 1000.0 * ctx.sample_rate as f32; // 20ms base
        let mod_depth_samples = self.params.depth_ms / 1000.0 * ctx.sample_rate as f32;

        let mut chorus_l = 0.0;
        let mut chorus_r = 0.0;

        // Multiple voices with phase offsets
        for voice in 0..self.params.voices.min(4) as usize {
            let voice_phase_offset = voice as f32 / self.params.voices as f32;
            let phase_l = (self.phase + voice_phase_offset) % 1.0;
            let phase_r = (self.phase + voice_phase_offset + self.params.stereo_spread * 0.25) % 1.0;

            let mod_l = (phase_l * TWO_PI).sin();
            let mod_r = (phase_r * TWO_PI).sin();

            let delay_l = base_delay_samples + mod_l * mod_depth_samples;
            let delay_r = base_delay_samples + mod_r * mod_depth_samples;

            chorus_l += self.buffer_left.read_interpolated(delay_l.max(1.0));
            chorus_r += self.buffer_right.read_interpolated(delay_r.max(1.0));
        }

        // Normalize by voice count
        let voice_gain = 1.0 / self.params.voices.max(1) as f32;
        chorus_l *= voice_gain;
        chorus_r *= voice_gain;

        // Advance phase
        advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);

        // Mix dry and wet
        StereoSample {
            left: input.left * (1.0 - self.params.mix) + chorus_l * self.params.mix,
            right: input.right * (1.0 - self.params.mix) + chorus_r * self.params.mix,
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.buffer_left.clear();
        self.buffer_right.clear();
    }

    fn name(&self) -> &'static str {
        "chorus"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// PHASER EFFECT
// ============================================================================
//
// Phaser uses a series of all-pass filters that sweep through frequencies,
// creating notches that move up and down the frequency spectrum.
// ============================================================================

/// Phaser effect
pub struct PhaserEffect {
    pub params: PhaserParams,
    pub active: bool,
    pub phase: f32,
    allpass_states: Vec<(f32, f32)>, // (left, right) for each stage
}

impl PhaserEffect {
    pub fn new() -> Self {
        Self {
            params: PhaserParams::default(),
            active: true,
            phase: 0.0,
            allpass_states: vec![(0.0, 0.0); 12], // Max 12 stages
        }
    }
}

/// First-order all-pass filter (free function to avoid borrow issues)
#[inline]
fn phaser_allpass(input: f32, delay: f32, state: &mut f32) -> f32 {
    let output = -input * delay + *state;
    *state = output * delay + input;
    output
}

impl Default for PhaserEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for PhaserEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.mix == 0.0 {
            return input;
        }

        // LFO for filter sweep
        let lfo = (self.phase * TWO_PI).sin();

        // Map LFO to all-pass coefficient (0.0 to 1.0 range)
        let min_coef = 0.1;
        let max_coef = 0.9;
        let coef = min_coef + (lfo * 0.5 + 0.5) * self.params.depth * (max_coef - min_coef);

        // Process through all-pass stages
        let mut left = input.left;
        let mut right = input.right;

        let stages = self.params.stages.min(12) as usize;
        for i in 0..stages {
            let (ref mut state_l, ref mut state_r) = self.allpass_states[i];
            left = phaser_allpass(left, coef, state_l);
            right = phaser_allpass(right, coef, state_r);
        }

        // Add feedback
        let feedback = self.params.feedback.clamp(-0.9, 0.9);

        // Advance phase
        advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);

        // Mix
        StereoSample {
            left: input.left * (1.0 - self.params.mix) + (left + input.left * feedback) * self.params.mix,
            right: input.right * (1.0 - self.params.mix) + (right + input.right * feedback) * self.params.mix,
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        for state in &mut self.allpass_states {
            *state = (0.0, 0.0);
        }
    }

    fn name(&self) -> &'static str {
        "phaser"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// FLANGER EFFECT
// ============================================================================
//
// Flanger is similar to chorus but with shorter delays and more feedback,
// creating a jet-like sweeping sound.
// ============================================================================

/// Flanger effect
pub struct FlangerEffect {
    pub params: FlangerParams,
    pub active: bool,
    pub phase: f32,
    buffer_left: EffectBuffer,
    buffer_right: EffectBuffer,
    feedback_left: f32,
    feedback_right: f32,
}

impl FlangerEffect {
    pub fn new(sample_rate: u32) -> Self {
        let buffer_size = (sample_rate as f32 * 0.02) as usize; // 20ms max
        Self {
            params: FlangerParams::default(),
            active: true,
            phase: 0.0,
            buffer_left: EffectBuffer::new(buffer_size),
            buffer_right: EffectBuffer::new(buffer_size),
            feedback_left: 0.0,
            feedback_right: 0.0,
        }
    }
}

impl Effect for FlangerEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.mix == 0.0 {
            return input;
        }

        // Write input + feedback to buffer
        self.buffer_left.write(input.left + self.feedback_left * self.params.feedback);
        self.buffer_right.write(input.right + self.feedback_right * self.params.feedback);

        // LFO modulation
        let lfo = (self.phase * TWO_PI).sin();
        let depth_samples = self.params.depth_ms / 1000.0 * ctx.sample_rate as f32;
        let min_delay = 1.0;
        let delay = min_delay + (lfo * 0.5 + 0.5) * depth_samples;

        // Read delayed samples
        let delayed_l = self.buffer_left.read_interpolated(delay);
        let delayed_r = self.buffer_right.read_interpolated(delay);

        // Store for feedback
        self.feedback_left = delayed_l;
        self.feedback_right = delayed_r;

        // Advance phase
        advance_phase(&mut self.phase, self.params.rate_hz, ctx.sample_rate);

        // Mix
        StereoSample {
            left: input.left * (1.0 - self.params.mix) + delayed_l * self.params.mix,
            right: input.right * (1.0 - self.params.mix) + delayed_r * self.params.mix,
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.buffer_left.clear();
        self.buffer_right.clear();
        self.feedback_left = 0.0;
        self.feedback_right = 0.0;
    }

    fn name(&self) -> &'static str {
        "flanger"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// AUTO-PAN EFFECT
// ============================================================================
//
// Automatically pans the signal left and right using an LFO.
// ============================================================================

/// Auto-pan effect
pub struct AutoPanEffect {
    pub rate_hz: f32,
    pub depth: f32,
    pub waveform: LfoWaveform,
    pub active: bool,
    pub mix: f32,
    phase: f32,
    random_state: u32,
}

impl AutoPanEffect {
    pub fn new() -> Self {
        Self {
            rate_hz: 1.0,
            depth: 0.5,
            waveform: LfoWaveform::Sine,
            active: true,
            mix: 1.0,
            phase: 0.0,
            random_state: 11111,
        }
    }
}

impl Default for AutoPanEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl Effect for AutoPanEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.depth == 0.0 {
            return input;
        }

        // LFO value for pan position
        let lfo = lfo_value(self.phase, self.waveform, &mut self.random_state);
        let pan = lfo * self.depth;

        // Equal-power pan
        let pan_normalized = (pan + 1.0) * 0.5;
        let angle = pan_normalized * PI * 0.5;
        let left_gain = angle.cos();
        let right_gain = angle.sin();

        // Advance phase
        advance_phase(&mut self.phase, self.rate_hz, ctx.sample_rate);

        let mono = input.to_mono();

        if self.mix >= 1.0 {
            StereoSample {
                left: mono * left_gain,
                right: mono * right_gain,
            }
        } else {
            StereoSample {
                left: input.left * (1.0 - self.mix) + mono * left_gain * self.mix,
                right: input.right * (1.0 - self.mix) + mono * right_gain * self.mix,
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn name(&self) -> &'static str {
        "autopan"
    }

    fn is_active(&self) -> bool {
        self.active && self.depth > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

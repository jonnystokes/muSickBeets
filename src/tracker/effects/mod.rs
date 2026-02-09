// ============================================================================
// EFFECTS MODULE - Unified Audio Effects System
// ============================================================================
//
// This module provides a unified effects system where all effects can work
// on both individual channels and the master bus.
//
// ARCHITECTURE:
// - Each effect implements the Effect trait
// - Effects process stereo sample pairs (left, right)
// - Effects maintain their own state (buffers, phases, etc.)
// - Effects can be chained in any order
//
// SUBMODULES:
// - state: Effect parameter and state structures
// - core: Basic effects (amplitude, pan)
// - modulation: Time-based modulation (vibrato, tremolo, chorus, phaser)
// - dynamics: Level processing (distortion, bitcrush, compressor)
// - spatial: Space effects (reverb, delay)
// - filters: Frequency filters (high-pass, low-pass, band-pass)
// - processor: Effect chain processing
//
// ============================================================================

pub mod state;
pub mod core;
pub mod modulation;
pub mod dynamics;
pub mod spatial;
pub mod filters;
pub mod processor;

use std::f32::consts::PI;

// ============================================================================
// CONSTANTS
// ============================================================================

pub const TWO_PI: f32 = std::f32::consts::TAU;

// ============================================================================
// STEREO SAMPLE
// ============================================================================

/// A stereo sample pair
#[derive(Clone, Copy, Debug, Default)]
pub struct StereoSample {
    pub left: f32,
    pub right: f32,
}

impl StereoSample {
    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    pub fn mono(value: f32) -> Self {
        Self { left: value, right: value }
    }

    pub fn to_mono(&self) -> f32 {
        (self.left + self.right) * 0.5
    }
}

// ============================================================================
// EFFECT CONTEXT
// ============================================================================

/// Context provided to effects during processing
#[derive(Clone, Debug)]
pub struct EffectContext {
    pub sample_rate: u32,
    pub current_sample: u64,
    pub max_buffer_samples: usize,
    pub input_frequency: f32,
}

impl EffectContext {
    pub fn new(sample_rate: u32, max_buffer_seconds: f32) -> Self {
        Self {
            sample_rate,
            current_sample: 0,
            max_buffer_samples: (sample_rate as f32 * max_buffer_seconds) as usize,
            input_frequency: 0.0,
        }
    }

    pub fn advance(&mut self) {
        self.current_sample = self.current_sample.wrapping_add(1);
    }
}

// ============================================================================
// EFFECT TRAIT
// ============================================================================

/// The core trait that all effects implement
pub trait Effect: Send + Sync {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample;
    fn reset(&mut self);
    fn name(&self) -> &'static str;
    fn is_active(&self) -> bool;
    fn set_mix(&mut self, mix: f32);
    fn get_mix(&self) -> f32;
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if x.abs() < 1.0 {
        x - (x * x * x) / 3.0
    } else {
        x.signum() * (2.0 / 3.0)
    }
}

#[inline]
pub fn hard_clip(x: f32, min: f32, max: f32) -> f32 {
    x.clamp(min, max)
}

#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.abs().max(1e-10).log10()
}

/// Calculate equal-power pan coefficients
/// Returns (left_gain, right_gain)
#[inline]
pub fn pan_coefficients(pan: f32) -> (f32, f32) {
    // pan: -1.0 = full left, 0.0 = center, 1.0 = full right
    let pan_normalized = (pan + 1.0) * 0.5; // 0.0 to 1.0
    let angle = pan_normalized * std::f32::consts::FRAC_PI_2; // 0 to PI/2
    (angle.cos(), angle.sin())
}

// ============================================================================
// CHANNEL EFFECT STATE
// ============================================================================
//
// Per-channel effect state that stores all effect parameters and buffers.
// Used by channels to process audio with effects.
// ============================================================================

/// Per-channel effect state
#[derive(Clone, Debug)]
pub struct ChannelEffectState {
    // Basic
    pub amplitude: f32,
    pub pan: f32,

    // Vibrato
    pub vibrato_rate_hz: f32,
    pub vibrato_depth_semitones: f32,
    pub vibrato_phase: f32,

    // Tremolo
    pub tremolo_rate_hz: f32,
    pub tremolo_depth: f32,
    pub tremolo_phase: f32,

    // Bitcrush
    pub bitcrush_bits: u8,

    // Distortion
    pub distortion_amount: f32,

    // Chorus
    pub chorus_mix: f32,
    pub chorus_rate_hz: f32,
    pub chorus_depth_ms: f32,
    pub chorus_feedback: f32,
    pub chorus_phase: f32,
    pub chorus_buffer: Vec<f32>,
    pub chorus_write_position: usize,
}

impl Default for ChannelEffectState {
    fn default() -> Self {
        Self {
            amplitude: 1.0,
            pan: 0.0,
            vibrato_rate_hz: 0.0,
            vibrato_depth_semitones: 0.0,
            vibrato_phase: 0.0,
            tremolo_rate_hz: 0.0,
            tremolo_depth: 0.0,
            tremolo_phase: 0.0,
            bitcrush_bits: 16,
            distortion_amount: 0.0,
            chorus_mix: 0.0,
            chorus_rate_hz: 0.0,
            chorus_depth_ms: 0.0,
            chorus_feedback: 0.0,
            chorus_phase: 0.0,
            chorus_buffer: Vec::new(),
            chorus_write_position: 0,
        }
    }
}

impl ChannelEffectState {
    pub fn initialize_chorus_buffer(&mut self, sample_rate: u32) {
        let max_delay_samples = ((50.0 / 1000.0) * sample_rate as f32) as usize + 1;
        self.chorus_buffer = vec![0.0; max_delay_samples];
        self.chorus_write_position = 0;
    }
}

// ============================================================================
// MASTER EFFECT STATE
// ============================================================================
//
// Master bus effect state with all reverb, delay, and chorus parameters.
// ============================================================================

/// Master bus effect state
#[derive(Clone, Debug)]
pub struct MasterEffectState {
    pub amplitude: f32,
    pub pan: f32,

    // Reverb 1 (simple)
    pub reverb1_enabled: bool,
    pub reverb1_room_size: f32,
    pub reverb1_mix: f32,
    pub reverb1_buffer: Vec<f32>,
    pub reverb1_position: usize,

    // Reverb 2 (advanced)
    pub reverb2_enabled: bool,
    pub reverb2_room_size: f32,
    pub reverb2_decay: f32,
    pub reverb2_damping: f32,
    pub reverb2_mix: f32,
    pub reverb2_predelay_ms: f32,
    pub reverb2_early_buffers: Vec<Vec<f32>>,
    pub reverb2_early_positions: Vec<usize>,
    pub reverb2_comb_buffers: Vec<Vec<f32>>,
    pub reverb2_comb_positions: Vec<usize>,
    pub reverb2_comb_filters: Vec<f32>,
    pub reverb2_allpass_buffers: Vec<Vec<f32>>,
    pub reverb2_allpass_positions: Vec<usize>,

    // Delay
    pub delay_enabled: bool,
    pub delay_time_samples: u32,
    pub delay_feedback: f32,
    pub delay_buffer_left: Vec<f32>,
    pub delay_buffer_right: Vec<f32>,
    pub delay_write_position: usize,

    // Chorus
    pub chorus_enabled: bool,
    pub chorus_mix: f32,
    pub chorus_rate_hz: f32,
    pub chorus_depth_ms: f32,
    pub chorus_phase: f32,
    pub chorus_stereo_spread: f32,
    pub chorus_buffer_left: Vec<f32>,
    pub chorus_buffer_right: Vec<f32>,
    pub chorus_write_position: usize,
}

impl MasterEffectState {
    pub fn new() -> Self {
        Self {
            amplitude: 1.0,
            pan: 0.0,

            reverb1_enabled: false,
            reverb1_room_size: 0.5,
            reverb1_mix: 0.3,
            reverb1_buffer: Vec::new(),
            reverb1_position: 0,

            reverb2_enabled: false,
            reverb2_room_size: 0.5,
            reverb2_decay: 0.5,
            reverb2_damping: 0.5,
            reverb2_mix: 0.3,
            reverb2_predelay_ms: 20.0,
            reverb2_early_buffers: Vec::new(),
            reverb2_early_positions: Vec::new(),
            reverb2_comb_buffers: Vec::new(),
            reverb2_comb_positions: Vec::new(),
            reverb2_comb_filters: Vec::new(),
            reverb2_allpass_buffers: Vec::new(),
            reverb2_allpass_positions: Vec::new(),

            delay_enabled: false,
            delay_time_samples: 12000,
            delay_feedback: 0.3,
            delay_buffer_left: Vec::new(),
            delay_buffer_right: Vec::new(),
            delay_write_position: 0,

            chorus_enabled: false,
            chorus_mix: 0.0,
            chorus_rate_hz: 1.0,
            chorus_depth_ms: 3.0,
            chorus_phase: 0.0,
            chorus_stereo_spread: 0.5,
            chorus_buffer_left: Vec::new(),
            chorus_buffer_right: Vec::new(),
            chorus_write_position: 0,
        }
    }

    pub fn initialize_buffers(&mut self, sample_rate: u32) {
        let max_buffer_size = (sample_rate as f32 * 2.0) as usize;

        // Reverb 1
        self.reverb1_buffer = vec![0.0; max_buffer_size];

        // Reverb 2 - early reflections
        let early_delay_times_ms = [7.0, 11.0, 13.0, 17.0, 19.0, 23.0];
        self.reverb2_early_buffers = early_delay_times_ms
            .iter()
            .map(|&ms| {
                let samples = ((ms / 1000.0) * sample_rate as f32 * 2.0) as usize;
                vec![0.0; samples.max(1)]
            })
            .collect();
        self.reverb2_early_positions = vec![0; early_delay_times_ms.len()];

        // Reverb 2 - comb filters
        let comb_delay_times_ms = [29.7, 37.1, 41.1, 43.7, 47.6, 53.0, 59.3, 67.0];
        self.reverb2_comb_buffers = comb_delay_times_ms
            .iter()
            .map(|&ms| {
                let samples = ((ms / 1000.0) * sample_rate as f32 * 2.0) as usize;
                vec![0.0; samples.max(1)]
            })
            .collect();
        self.reverb2_comb_positions = vec![0; comb_delay_times_ms.len()];
        self.reverb2_comb_filters = vec![0.0; comb_delay_times_ms.len()];

        // Reverb 2 - all-pass filters
        let allpass_delay_times_ms = [5.0, 1.7];
        self.reverb2_allpass_buffers = allpass_delay_times_ms
            .iter()
            .map(|&ms| {
                let samples = ((ms / 1000.0) * sample_rate as f32) as usize;
                vec![0.0; samples.max(1)]
            })
            .collect();
        self.reverb2_allpass_positions = vec![0; allpass_delay_times_ms.len()];

        // Delay
        self.delay_buffer_left = vec![0.0; max_buffer_size];
        self.delay_buffer_right = vec![0.0; max_buffer_size];

        // Chorus
        let chorus_buffer_size = ((50.0 / 1000.0) * sample_rate as f32) as usize + 1;
        self.chorus_buffer_left = vec![0.0; chorus_buffer_size];
        self.chorus_buffer_right = vec![0.0; chorus_buffer_size];
    }
}

impl Default for MasterEffectState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CHANNEL EFFECT PROCESSING
// ============================================================================

/// Applies channel effects to a mono sample and returns stereo (left, right)
pub fn apply_channel_effects(
    input_sample: f32,
    effects: &mut ChannelEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    let mut sample = input_sample;

    // Chorus
    if effects.chorus_mix > 0.0 && effects.chorus_rate_hz > 0.0 {
        sample = apply_mono_chorus(sample, effects, sample_rate);
    }

    // Tremolo
    if effects.tremolo_rate_hz > 0.0 && effects.tremolo_depth > 0.0 {
        let lfo = effects.tremolo_phase.sin();
        let amplitude_modulation = 1.0 - effects.tremolo_depth * (1.0 - lfo) / 2.0;
        sample *= amplitude_modulation;

        effects.tremolo_phase += TWO_PI * effects.tremolo_rate_hz / sample_rate as f32;
        if effects.tremolo_phase >= TWO_PI {
            effects.tremolo_phase -= TWO_PI;
        }
    }

    // Bitcrush
    if effects.bitcrush_bits < 16 {
        let quantization_levels = 2.0_f32.powi(effects.bitcrush_bits as i32);
        sample = (sample * quantization_levels).round() / quantization_levels;
    }

    // Distortion
    if effects.distortion_amount > 0.0 {
        let drive = 1.0 + effects.distortion_amount * 10.0;
        let driven_sample = sample * drive;
        sample = driven_sample / (1.0 + driven_sample.abs());
    }

    // Amplitude
    sample *= effects.amplitude;

    // Pan (constant-power)
    let pan_left_coefficient = ((1.0 - effects.pan) * 0.5).sqrt();
    let pan_right_coefficient = ((1.0 + effects.pan) * 0.5).sqrt();

    (sample * pan_left_coefficient, sample * pan_right_coefficient)
}

/// Calculate vibrato frequency multiplier
pub fn calculate_vibrato_multiplier(effects: &mut ChannelEffectState, sample_rate: u32) -> f32 {
    if effects.vibrato_rate_hz > 0.0 && effects.vibrato_depth_semitones > 0.0 {
        let lfo = effects.vibrato_phase.sin();
        let frequency_multiplier = 2.0_f32.powf(lfo * effects.vibrato_depth_semitones / 12.0);

        effects.vibrato_phase += TWO_PI * effects.vibrato_rate_hz / sample_rate as f32;
        if effects.vibrato_phase >= TWO_PI {
            effects.vibrato_phase -= TWO_PI;
        }

        frequency_multiplier
    } else {
        1.0
    }
}

/// Apply mono chorus effect
fn apply_mono_chorus(
    input_sample: f32,
    effects: &mut ChannelEffectState,
    sample_rate: u32,
) -> f32 {
    if effects.chorus_buffer.is_empty() {
        return input_sample;
    }

    let buffer_len = effects.chorus_buffer.len();
    let base_delay_ms = 7.0;
    let lfo = effects.chorus_phase.sin();
    let modulated_delay_ms = base_delay_ms + lfo * effects.chorus_depth_ms;
    let delay_samples = (modulated_delay_ms / 1000.0 * sample_rate as f32).max(1.0);

    let delay_samples_int = delay_samples as usize;
    let delay_samples_frac = delay_samples - delay_samples_int as f32;

    let read_pos_1 = (effects.chorus_write_position + buffer_len - delay_samples_int) % buffer_len;
    let read_pos_2 = (read_pos_1 + buffer_len - 1) % buffer_len;

    let delayed_sample = lerp(
        effects.chorus_buffer[read_pos_1],
        effects.chorus_buffer[read_pos_2],
        delay_samples_frac,
    );

    effects.chorus_buffer[effects.chorus_write_position] =
        input_sample + delayed_sample * effects.chorus_feedback;
    effects.chorus_write_position = (effects.chorus_write_position + 1) % buffer_len;

    effects.chorus_phase += TWO_PI * effects.chorus_rate_hz / sample_rate as f32;
    if effects.chorus_phase >= TWO_PI {
        effects.chorus_phase -= TWO_PI;
    }

    lerp(input_sample, delayed_sample, effects.chorus_mix)
}

// ============================================================================
// MASTER EFFECT PROCESSING
// ============================================================================

/// Applies all master effects to a stereo signal
pub fn apply_master_effects(
    mut left: f32,
    mut right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    // Reverb 1
    if effects.reverb1_enabled && effects.reverb1_mix > 0.001 {
        let (l, r) = apply_reverb1(left, right, effects, sample_rate);
        left = l;
        right = r;
    }

    // Reverb 2
    if effects.reverb2_enabled && effects.reverb2_mix > 0.001 {
        let (l, r) = apply_reverb2(left, right, effects, sample_rate);
        left = l;
        right = r;
    }

    // Delay
    if effects.delay_enabled && effects.delay_feedback > 0.001 {
        let (l, r) = apply_delay(left, right, effects);
        left = l;
        right = r;
    }

    // Chorus
    if effects.chorus_enabled && effects.chorus_mix > 0.001 {
        let (l, r) = apply_master_chorus(left, right, effects, sample_rate);
        left = l;
        right = r;
    }

    // Master amplitude
    left *= effects.amplitude;
    right *= effects.amplitude;

    // Master pan
    if effects.pan != 0.0 {
        let pan_left = ((1.0 - effects.pan) * 0.5).sqrt();
        let pan_right = ((1.0 + effects.pan) * 0.5).sqrt();
        left *= pan_left;
        right *= pan_right;
    }

    (left, right)
}

fn apply_reverb1(
    left: f32,
    right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    if effects.reverb1_buffer.is_empty() {
        return (left, right);
    }

    let delay_samples = (effects.reverb1_room_size * sample_rate as f32 * 0.05) as usize;
    let delay_samples = delay_samples.min(effects.reverb1_buffer.len() - 1).max(1);

    let read_pos = (effects.reverb1_position + effects.reverb1_buffer.len() - delay_samples)
        % effects.reverb1_buffer.len();
    let reverb_sample = effects.reverb1_buffer[read_pos];

    let mono_input = (left + right) * 0.5;
    effects.reverb1_buffer[effects.reverb1_position] = mono_input + reverb_sample * 0.5;
    effects.reverb1_position = (effects.reverb1_position + 1) % effects.reverb1_buffer.len();

    let wet = reverb_sample * effects.reverb1_mix;
    let dry = 1.0 - effects.reverb1_mix;

    (left * dry + wet, right * dry + wet)
}

fn apply_reverb2(
    left: f32,
    right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    if effects.reverb2_comb_buffers.is_empty() {
        return (left, right);
    }

    let mono_input = (left + right) * 0.5;
    let room_scale = 0.3 + effects.reverb2_room_size * 0.7;

    // Early reflections
    let mut early_reflections = 0.0;
    for i in 0..effects.reverb2_early_buffers.len() {
        if effects.reverb2_early_buffers[i].is_empty() {
            continue;
        }

        let buffer_len = effects.reverb2_early_buffers[i].len();
        let delay = ((buffer_len as f32 * room_scale) as usize).min(buffer_len - 1).max(1);

        let read_pos = (effects.reverb2_early_positions[i] + buffer_len - delay) % buffer_len;
        early_reflections += effects.reverb2_early_buffers[i][read_pos] * (0.7_f32.powi(i as i32 + 1));

        effects.reverb2_early_buffers[i][effects.reverb2_early_positions[i]] = mono_input;
        effects.reverb2_early_positions[i] = (effects.reverb2_early_positions[i] + 1) % buffer_len;
    }
    early_reflections /= effects.reverb2_early_buffers.len() as f32;

    // Comb filters
    let mut comb_output = 0.0;
    let target_decay_samples = effects.reverb2_decay * sample_rate as f32;

    for i in 0..effects.reverb2_comb_buffers.len() {
        if effects.reverb2_comb_buffers[i].is_empty() {
            continue;
        }

        let buffer_len = effects.reverb2_comb_buffers[i].len();
        let delay = ((buffer_len as f32 * room_scale) as usize).min(buffer_len - 1).max(1);

        let read_pos = (effects.reverb2_comb_positions[i] + buffer_len - delay) % buffer_len;
        let delayed = effects.reverb2_comb_buffers[i][read_pos];

        effects.reverb2_comb_filters[i] = lerp(
            delayed,
            effects.reverb2_comb_filters[i],
            effects.reverb2_damping,
        );
        let filtered = effects.reverb2_comb_filters[i];

        let delay_time = delay as f32 / sample_rate as f32;
        let feedback = if target_decay_samples > 0.0 {
            10.0_f32.powf(-3.0 * delay_time / effects.reverb2_decay).min(0.98)
        } else {
            0.5
        };

        let input_with_early = mono_input + early_reflections * 0.3;
        effects.reverb2_comb_buffers[i][effects.reverb2_comb_positions[i]] =
            input_with_early + filtered * feedback;
        effects.reverb2_comb_positions[i] = (effects.reverb2_comb_positions[i] + 1) % buffer_len;

        comb_output += delayed;
    }
    comb_output /= effects.reverb2_comb_buffers.len() as f32;

    // All-pass filters
    let mut allpass_output = comb_output;
    let allpass_gain = 0.5;

    for i in 0..effects.reverb2_allpass_buffers.len() {
        if effects.reverb2_allpass_buffers[i].is_empty() {
            continue;
        }

        let buffer_len = effects.reverb2_allpass_buffers[i].len();
        let read_pos = (effects.reverb2_allpass_positions[i] + buffer_len - (buffer_len - 1))
            % buffer_len;

        let delayed = effects.reverb2_allpass_buffers[i][read_pos];
        let output = -allpass_output * allpass_gain + delayed;
        effects.reverb2_allpass_buffers[i][effects.reverb2_allpass_positions[i]] =
            allpass_output + delayed * allpass_gain;
        effects.reverb2_allpass_positions[i] =
            (effects.reverb2_allpass_positions[i] + 1) % buffer_len;

        allpass_output = output;
    }

    let wet = allpass_output * effects.reverb2_mix;
    let dry = 1.0 - effects.reverb2_mix;

    (soft_clip(left * dry + wet), soft_clip(right * dry + wet))
}

fn apply_delay(
    left: f32,
    right: f32,
    effects: &mut MasterEffectState,
) -> (f32, f32) {
    if effects.delay_buffer_left.is_empty() {
        return (left, right);
    }

    let buffer_len = effects.delay_buffer_left.len();
    let delay_samples = (effects.delay_time_samples as usize).min(buffer_len - 1).max(1);

    let read_pos = (effects.delay_write_position + buffer_len - delay_samples) % buffer_len;
    let delayed_left = effects.delay_buffer_left[read_pos];
    let delayed_right = effects.delay_buffer_right[read_pos];

    effects.delay_buffer_left[effects.delay_write_position] =
        left + delayed_left * effects.delay_feedback;
    effects.delay_buffer_right[effects.delay_write_position] =
        right + delayed_right * effects.delay_feedback;
    effects.delay_write_position = (effects.delay_write_position + 1) % buffer_len;

    (left + delayed_left * 0.5, right + delayed_right * 0.5)
}

fn apply_master_chorus(
    left: f32,
    right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    if effects.chorus_buffer_left.is_empty() {
        return (left, right);
    }

    let buffer_len = effects.chorus_buffer_left.len();
    let base_delay_ms = 7.0;

    let lfo_left = effects.chorus_phase.sin();
    let modulated_delay_left = base_delay_ms + lfo_left * effects.chorus_depth_ms;
    let delay_samples_left = (modulated_delay_left / 1000.0 * sample_rate as f32).max(1.0);

    let lfo_right = (effects.chorus_phase + PI * effects.chorus_stereo_spread).sin();
    let modulated_delay_right = base_delay_ms + lfo_right * effects.chorus_depth_ms;
    let delay_samples_right = (modulated_delay_right / 1000.0 * sample_rate as f32).max(1.0);

    // Left channel
    let delay_int_left = delay_samples_left as usize;
    let delay_frac_left = delay_samples_left - delay_int_left as f32;
    let read_pos_1_left = (effects.chorus_write_position + buffer_len - delay_int_left) % buffer_len;
    let read_pos_2_left = (read_pos_1_left + buffer_len - 1) % buffer_len;
    let delayed_left = lerp(
        effects.chorus_buffer_left[read_pos_1_left],
        effects.chorus_buffer_left[read_pos_2_left],
        delay_frac_left,
    );

    // Right channel
    let delay_int_right = delay_samples_right as usize;
    let delay_frac_right = delay_samples_right - delay_int_right as f32;
    let read_pos_1_right = (effects.chorus_write_position + buffer_len - delay_int_right) % buffer_len;
    let read_pos_2_right = (read_pos_1_right + buffer_len - 1) % buffer_len;
    let delayed_right = lerp(
        effects.chorus_buffer_right[read_pos_1_right],
        effects.chorus_buffer_right[read_pos_2_right],
        delay_frac_right,
    );

    effects.chorus_buffer_left[effects.chorus_write_position] = left;
    effects.chorus_buffer_right[effects.chorus_write_position] = right;
    effects.chorus_write_position = (effects.chorus_write_position + 1) % buffer_len;

    effects.chorus_phase += TWO_PI * effects.chorus_rate_hz / sample_rate as f32;
    if effects.chorus_phase >= TWO_PI {
        effects.chorus_phase -= TWO_PI;
    }

    (
        lerp(left, delayed_left, effects.chorus_mix),
        lerp(right, delayed_right, effects.chorus_mix),
    )
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_channel_effects() {
        let effects = ChannelEffectState::default();
        assert_eq!(effects.amplitude, 1.0);
        assert_eq!(effects.pan, 0.0);
        assert_eq!(effects.bitcrush_bits, 16);
    }

    #[test]
    fn test_channel_effects_processing() {
        let mut effects = ChannelEffectState::default();
        effects.amplitude = 0.5;
        let (left, right) = apply_channel_effects(1.0, &mut effects, 48000);
        // With center pan (0.0), both channels get sqrt(0.5) ≈ 0.707
        // So 0.5 * 0.707 ≈ 0.354
        let expected = 0.5 * (0.5_f32).sqrt();
        assert!((left - expected).abs() < 0.01);
        assert!((right - expected).abs() < 0.01);
    }

    #[test]
    fn test_master_effects_creation() {
        let mut effects = MasterEffectState::new();
        effects.initialize_buffers(48000);
        assert!(!effects.reverb1_buffer.is_empty());
        assert!(!effects.delay_buffer_left.is_empty());
    }
}

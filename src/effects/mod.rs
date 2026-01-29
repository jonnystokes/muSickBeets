// ============================================================================
// EFFECTS MODULE - Unified Audio Effects System
// ============================================================================
//
// This module provides a unified effects system where all effects can work
// on both individual channels and the master bus. Effects are processed
// in the order they are listed in the song file.
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
// - pitch: Pitch effects (pitch shift, time stretch)
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

// Re-export commonly used types
pub use state::{EffectState, EffectParameters, EffectBuffer};
pub use processor::{EffectChain, EffectType, process_effect_chain};

// ============================================================================
// BACKWARD COMPATIBILITY
// ============================================================================
//
// Re-export types from effects_legacy for backward compatibility with
// existing channel.rs, master_bus.rs, and parser.rs code.
// ============================================================================

pub use crate::effects_legacy::{
    ChannelEffectState,
    MasterEffectState,
    apply_channel_effects,
    apply_master_effects,
    calculate_vibrato_multiplier,
    find_channel_effect_by_name,
    find_master_effect_by_name,
    CHANNEL_EFFECT_REGISTRY,
    MASTER_EFFECT_REGISTRY,
    EffectDefinition,
    EffectParameter,
};

// ============================================================================
// EFFECT TRAIT
// ============================================================================
//
// All effects implement this trait, allowing them to be used uniformly
// regardless of whether they're applied to a channel or the master bus.
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

/// Context provided to effects during processing
/// Contains sample rate and other global parameters
#[derive(Clone, Debug)]
pub struct EffectContext {
    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Current time in samples (for LFOs, etc.)
    pub current_sample: u64,

    /// Maximum buffer size for delay-based effects
    pub max_buffer_samples: usize,

    /// Input frequency (for vibrato, etc.) - 0 if not available
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

    pub fn with_frequency(mut self, freq: f32) -> Self {
        self.input_frequency = freq;
        self
    }

    pub fn advance(&mut self) {
        self.current_sample = self.current_sample.wrapping_add(1);
    }

    pub fn time_seconds(&self) -> f32 {
        self.current_sample as f32 / self.sample_rate as f32
    }
}

/// The core trait that all effects implement
pub trait Effect: Send + Sync {
    /// Process a stereo sample pair
    /// Returns the processed sample
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample;

    /// Reset the effect state (clear buffers, reset phases)
    fn reset(&mut self);

    /// Get the effect name for display/debugging
    fn name(&self) -> &'static str;

    /// Check if the effect is currently active (not bypassed)
    fn is_active(&self) -> bool;

    /// Set the wet/dry mix (0.0 = fully dry, 1.0 = fully wet)
    fn set_mix(&mut self, mix: f32);

    /// Get the current wet/dry mix
    fn get_mix(&self) -> f32;
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Linear interpolation between two values
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Soft clipping using tanh-style saturation
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if x.abs() < 1.0 {
        x - (x * x * x) / 3.0
    } else {
        x.signum() * (2.0 / 3.0)
    }
}

/// Hard clipping
#[inline]
pub fn hard_clip(x: f32, min: f32, max: f32) -> f32 {
    x.clamp(min, max)
}

/// Convert decibels to linear amplitude
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert linear amplitude to decibels
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

/// Fast sine approximation for LFOs (less accurate but faster)
#[inline]
pub fn fast_sin(x: f32) -> f32 {
    // Normalize to -PI to PI
    let x = x % std::f32::consts::TAU;
    let x = if x > std::f32::consts::PI {
        x - std::f32::consts::TAU
    } else if x < -std::f32::consts::PI {
        x + std::f32::consts::TAU
    } else {
        x
    };

    // Polynomial approximation
    let x2 = x * x;
    x * (1.0 - x2 * (0.16666667 - x2 * 0.00833333))
}

// ============================================================================
// CONSTANTS
// ============================================================================

pub const TWO_PI: f32 = std::f32::consts::TAU;
pub const PI: f32 = std::f32::consts::PI;

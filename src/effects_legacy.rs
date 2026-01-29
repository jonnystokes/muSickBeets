// ============================================================================
// EFFECTS.RS - Audio Effects Registry and Processing
// ============================================================================
//
// This module provides audio effects that modify the sound after it's generated.
// Effects can be applied per-channel or on the master bus.
//
// WHAT ARE AUDIO EFFECTS?
// Effects transform audio signals. They can add space (reverb), rhythm (delay),
// warmth (saturation), or texture (chorus, bitcrush). Effects are the "spices"
// that give synthesized sounds character and depth.
//
// EFFECT TYPES:
// - Channel Effects: Applied to individual channels before mixing
//   Examples: amplitude, pan, vibrato, tremolo, bitcrush, distortion
//
// - Master Effects: Applied to the mixed signal of all channels
//   Examples: reverb, delay, master amplitude, master pan
//
// HOW TO ADD A NEW EFFECT:
// 1. For channel effects: Add to CHANNEL_EFFECT_REGISTRY and implement the processing
// 2. For master effects: Add to MASTER_EFFECT_REGISTRY and implement the processing
// 3. Each effect has a definition (metadata) and a processing function
//
// REGISTRY PATTERN:
// Effects are defined in registries (arrays of definitions). Each definition includes:
// - ID: Unique identifier
// - Name: What you type in the CSV file
// - Parameters: What values the effect accepts
// - Capabilities: Whether it works on channels, master, or both
// ============================================================================

use std::f32::consts::PI;
use crate::helper::{lerp, TWO_PI, soft_clip};

// ============================================================================
// EFFECT STATE STRUCTURES
// ============================================================================
//
// These structures hold the current values of all effect parameters.
// They're stored in each channel and the master bus.
// ============================================================================

/// Per-channel effect state
/// Contains all the parameters that modify a single channel's sound
#[derive(Clone, Debug)]
pub struct ChannelEffectState {
    /// Volume level from 0.0 (silent) to 1.0 (full volume)
    pub amplitude: f32,

    /// Stereo position from -1.0 (full left) to 1.0 (full right), 0.0 is center
    pub pan: f32,

    /// Vibrato rate in Hz (how fast the pitch wobbles)
    pub vibrato_rate_hz: f32,

    /// Vibrato depth in semitones (how much the pitch wobbles)
    pub vibrato_depth_semitones: f32,

    /// Current phase of the vibrato LFO (internal state, 0 to 2*PI)
    pub vibrato_phase: f32,

    /// Tremolo rate in Hz (how fast the volume wobbles)
    pub tremolo_rate_hz: f32,

    /// Tremolo depth from 0.0 (no effect) to 1.0 (full effect)
    pub tremolo_depth: f32,

    /// Current phase of the tremolo LFO (internal state, 0 to 2*PI)
    pub tremolo_phase: f32,

    /// Bitcrush bits from 1 (extreme crush) to 16 (no effect)
    pub bitcrush_bits: u8,

    /// Distortion amount from 0.0 (clean) to 1.0 (heavy distortion)
    pub distortion_amount: f32,

    /// Chorus mix from 0.0 (dry) to 1.0 (wet)
    pub chorus_mix: f32,

    /// Chorus rate in Hz (how fast the chorus modulates)
    pub chorus_rate_hz: f32,

    /// Chorus depth in milliseconds (max delay time variation)
    pub chorus_depth_ms: f32,

    /// Chorus feedback amount (0.0 to 0.9)
    pub chorus_feedback: f32,

    /// Chorus LFO phase (internal state)
    pub chorus_phase: f32,

    /// Chorus delay buffer (internal state)
    pub chorus_buffer: Vec<f32>,

    /// Chorus buffer write position (internal state)
    pub chorus_write_position: usize,
}

impl Default for ChannelEffectState {
    /// Creates a new effect state with all effects at their neutral/off settings
    fn default() -> Self {
        Self {
            amplitude: 1.0,                    // Full volume
            pan: 0.0,                          // Center
            vibrato_rate_hz: 0.0,              // Off
            vibrato_depth_semitones: 0.0,      // Off
            vibrato_phase: 0.0,
            tremolo_rate_hz: 0.0,              // Off
            tremolo_depth: 0.0,                // Off
            tremolo_phase: 0.0,
            bitcrush_bits: 16,                 // No crushing (full resolution)
            distortion_amount: 0.0,            // Clean
            chorus_mix: 0.0,                   // Off
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
    /// Initialize the chorus buffer for a given sample rate
    /// The buffer needs to be big enough for the maximum delay time
    pub fn initialize_chorus_buffer(&mut self, sample_rate: u32) {
        // Max chorus delay is about 50ms, so allocate enough buffer
        let max_delay_samples = ((50.0 / 1000.0) * sample_rate as f32) as usize + 1;
        self.chorus_buffer = vec![0.0; max_delay_samples];
        self.chorus_write_position = 0;
    }
}

/// Master bus effect state
/// Contains all the parameters that modify the mixed output
#[derive(Clone, Debug)]
pub struct MasterEffectState {
    /// Master volume level
    pub amplitude: f32,

    /// Master stereo position
    pub pan: f32,

    // ---- Reverb 1 (Simple delay-based) ----

    /// Whether reverb 1 is enabled
    pub reverb1_enabled: bool,

    /// Reverb 1 room size (affects delay length)
    pub reverb1_room_size: f32,

    /// Reverb 1 wet/dry mix
    pub reverb1_mix: f32,

    /// Reverb 1 internal buffer
    pub reverb1_buffer: Vec<f32>,

    /// Reverb 1 buffer write position
    pub reverb1_position: usize,

    // ---- Reverb 2 (Advanced multi-tap) ----

    /// Whether reverb 2 is enabled
    pub reverb2_enabled: bool,

    /// Reverb 2 room size (0.0 to 1.0)
    pub reverb2_room_size: f32,

    /// Reverb 2 decay time (how long the reverb tail lasts)
    pub reverb2_decay: f32,

    /// Reverb 2 damping (high frequency absorption, 0.0 to 1.0)
    pub reverb2_damping: f32,

    /// Reverb 2 wet/dry mix
    pub reverb2_mix: f32,

    /// Reverb 2 pre-delay in milliseconds
    pub reverb2_predelay_ms: f32,

    /// Reverb 2 early reflection buffers (multiple delay lines at prime-number lengths)
    pub reverb2_early_buffers: Vec<Vec<f32>>,

    /// Reverb 2 early reflection write positions
    pub reverb2_early_positions: Vec<usize>,

    /// Reverb 2 late reflection buffers (comb filters)
    pub reverb2_comb_buffers: Vec<Vec<f32>>,

    /// Reverb 2 comb filter write positions
    pub reverb2_comb_positions: Vec<usize>,

    /// Reverb 2 comb filter feedback states (for damping)
    pub reverb2_comb_filters: Vec<f32>,

    /// Reverb 2 all-pass filter buffers
    pub reverb2_allpass_buffers: Vec<Vec<f32>>,

    /// Reverb 2 all-pass filter write positions
    pub reverb2_allpass_positions: Vec<usize>,

    // ---- Delay ----

    /// Whether delay is enabled
    pub delay_enabled: bool,

    /// Delay time in samples
    pub delay_time_samples: u32,

    /// Delay feedback (echo repetition)
    pub delay_feedback: f32,

    /// Delay left channel buffer
    pub delay_buffer_left: Vec<f32>,

    /// Delay right channel buffer
    pub delay_buffer_right: Vec<f32>,

    /// Delay buffer write position
    pub delay_write_position: usize,

    // ---- Chorus (Master) ----

    /// Whether master chorus is enabled
    pub chorus_enabled: bool,

    /// Master chorus mix
    pub chorus_mix: f32,

    /// Master chorus rate
    pub chorus_rate_hz: f32,

    /// Master chorus depth
    pub chorus_depth_ms: f32,

    /// Master chorus phase
    pub chorus_phase: f32,

    /// Master chorus stereo spread (how much L/R phases differ)
    pub chorus_stereo_spread: f32,

    /// Master chorus left buffer
    pub chorus_buffer_left: Vec<f32>,

    /// Master chorus right buffer
    pub chorus_buffer_right: Vec<f32>,

    /// Master chorus write position
    pub chorus_write_position: usize,
}

impl MasterEffectState {
    /// Creates a new master effect state with default values
    /// Buffer allocation should be done separately with initialize_buffers()
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
            delay_time_samples: 12000, // 0.25 seconds at 48kHz
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

    /// Allocates all the buffers needed for master effects
    /// Call this once when the engine is initialized
    pub fn initialize_buffers(&mut self, sample_rate: u32) {
        // Maximum delay/reverb time is 2 seconds
        let max_buffer_size = (sample_rate as f32 * 2.0) as usize;

        // Reverb 1 buffer
        self.reverb1_buffer = vec![0.0; max_buffer_size];

        // Reverb 2 buffers - prime number lengths for early reflections
        // These create the initial sense of space
        let early_delay_times_ms = [7.0, 11.0, 13.0, 17.0, 19.0, 23.0];
        self.reverb2_early_buffers = early_delay_times_ms
            .iter()
            .map(|&ms| {
                let samples = ((ms / 1000.0) * sample_rate as f32 * 2.0) as usize; // 2x for room size scaling
                vec![0.0; samples.max(1)]
            })
            .collect();
        self.reverb2_early_positions = vec![0; early_delay_times_ms.len()];

        // Comb filter delays (create the reverb tail density)
        // Using Schroeder reverb style prime-number-like delays
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

        // All-pass filters (diffuse the reverb, make it smoother)
        let allpass_delay_times_ms = [5.0, 1.7];
        self.reverb2_allpass_buffers = allpass_delay_times_ms
            .iter()
            .map(|&ms| {
                let samples = ((ms / 1000.0) * sample_rate as f32) as usize;
                vec![0.0; samples.max(1)]
            })
            .collect();
        self.reverb2_allpass_positions = vec![0; allpass_delay_times_ms.len()];

        // Delay buffers
        self.delay_buffer_left = vec![0.0; max_buffer_size];
        self.delay_buffer_right = vec![0.0; max_buffer_size];

        // Chorus buffers
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
// EFFECT PARAMETER DEFINITION
// ============================================================================

/// Describes a parameter that an effect accepts
#[derive(Clone, Debug)]
pub struct EffectParameter {
    /// Parameter name (used in CSV, e.g., "room" for rv:room'mix)
    pub name: &'static str,

    /// Minimum allowed value
    pub min_value: f32,

    /// Maximum allowed value
    pub max_value: f32,

    /// Default value
    pub default_value: f32,

    /// Description of what this parameter does
    pub description: &'static str,
}

// ============================================================================
// EFFECT DEFINITION (REGISTRY PATTERN)
// ============================================================================

/// Defines an effect with its properties
#[derive(Clone)]
pub struct EffectDefinition {
    /// Unique identifier
    pub id: usize,

    /// Primary name (what you type in CSV)
    pub name: &'static str,

    /// Short name/alias
    pub short_name: &'static str,

    /// Whether this effect can be used on individual channels
    pub works_on_channel: bool,

    /// Whether this effect can be used on the master bus
    pub works_on_master: bool,

    /// Parameters this effect accepts
    pub parameters: &'static [EffectParameter],

    /// Description of the effect
    pub description: &'static str,
}

// ============================================================================
// CHANNEL EFFECT REGISTRY
// ============================================================================

/// Registry of all channel effects
pub static CHANNEL_EFFECT_REGISTRY: &[EffectDefinition] = &[
    EffectDefinition {
        id: 0,
        name: "amplitude",
        short_name: "a",
        works_on_channel: true,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "level",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 1.0,
                description: "Volume level",
            },
        ],
        description: "Volume/amplitude control",
    },
    EffectDefinition {
        id: 1,
        name: "pan",
        short_name: "p",
        works_on_channel: true,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "position",
                min_value: -1.0,
                max_value: 1.0,
                default_value: 0.0,
                description: "Stereo position: -1 = left, 0 = center, 1 = right",
            },
        ],
        description: "Stereo panning",
    },
    EffectDefinition {
        id: 2,
        name: "vibrato",
        short_name: "v",
        works_on_channel: true,
        works_on_master: false,
        parameters: &[
            EffectParameter {
                name: "rate",
                min_value: 0.0,
                max_value: 50.0,
                default_value: 0.0,
                description: "Vibrato speed in Hz",
            },
            EffectParameter {
                name: "depth",
                min_value: 0.0,
                max_value: 12.0,
                default_value: 0.0,
                description: "Vibrato depth in semitones",
            },
        ],
        description: "Pitch vibrato (pitch wobble)",
    },
    EffectDefinition {
        id: 3,
        name: "tremolo",
        short_name: "t",
        works_on_channel: true,
        works_on_master: false,
        parameters: &[
            EffectParameter {
                name: "rate",
                min_value: 0.0,
                max_value: 50.0,
                default_value: 0.0,
                description: "Tremolo speed in Hz",
            },
            EffectParameter {
                name: "depth",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
                description: "Tremolo intensity",
            },
        ],
        description: "Volume tremolo (volume wobble)",
    },
    EffectDefinition {
        id: 4,
        name: "bitcrush",
        short_name: "b",
        works_on_channel: true,
        works_on_master: false,
        parameters: &[
            EffectParameter {
                name: "bits",
                min_value: 1.0,
                max_value: 16.0,
                default_value: 16.0,
                description: "Bit depth (1-16, 16 = no effect)",
            },
        ],
        description: "Bit reduction for lo-fi sound",
    },
    EffectDefinition {
        id: 5,
        name: "distortion",
        short_name: "d",
        works_on_channel: true,
        works_on_master: false,
        parameters: &[
            EffectParameter {
                name: "amount",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
                description: "Distortion intensity",
            },
        ],
        description: "Soft clipping distortion",
    },
    EffectDefinition {
        id: 6,
        name: "chorus",
        short_name: "ch",
        works_on_channel: true,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "mix",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
                description: "Wet/dry mix",
            },
            EffectParameter {
                name: "rate",
                min_value: 0.1,
                max_value: 5.0,
                default_value: 1.0,
                description: "Modulation rate in Hz",
            },
            EffectParameter {
                name: "depth",
                min_value: 0.5,
                max_value: 10.0,
                default_value: 3.0,
                description: "Modulation depth in milliseconds",
            },
            EffectParameter {
                name: "feedback",
                min_value: 0.0,
                max_value: 0.9,
                default_value: 0.2,
                description: "Feedback amount",
            },
        ],
        description: "Chorus effect - creates richness and width",
    },
];

// ============================================================================
// MASTER EFFECT REGISTRY
// ============================================================================

/// Registry of master bus effects
pub static MASTER_EFFECT_REGISTRY: &[EffectDefinition] = &[
    EffectDefinition {
        id: 0,
        name: "reverb",
        short_name: "rv",
        works_on_channel: false,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "room",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.5,
                description: "Room size",
            },
            EffectParameter {
                name: "mix",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.3,
                description: "Wet/dry mix",
            },
        ],
        description: "Reverb 1 - Simple delay-based reverb (original)",
    },
    EffectDefinition {
        id: 1,
        name: "reverb2",
        short_name: "rv2",
        works_on_channel: false,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "room",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.5,
                description: "Room size",
            },
            EffectParameter {
                name: "decay",
                min_value: 0.1,
                max_value: 10.0,
                default_value: 2.0,
                description: "Decay time in seconds",
            },
            EffectParameter {
                name: "damping",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.5,
                description: "High frequency damping",
            },
            EffectParameter {
                name: "mix",
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.3,
                description: "Wet/dry mix",
            },
            EffectParameter {
                name: "predelay",
                min_value: 0.0,
                max_value: 100.0,
                default_value: 20.0,
                description: "Pre-delay in milliseconds",
            },
        ],
        description: "Reverb 2 - Advanced algorithmic reverb with comb/allpass filters",
    },
    EffectDefinition {
        id: 2,
        name: "delay",
        short_name: "dl",
        works_on_channel: false,
        works_on_master: true,
        parameters: &[
            EffectParameter {
                name: "time",
                min_value: 0.01,
                max_value: 2.0,
                default_value: 0.25,
                description: "Delay time in seconds",
            },
            EffectParameter {
                name: "feedback",
                min_value: 0.0,
                max_value: 0.95,
                default_value: 0.3,
                description: "Feedback amount (echo repetitions)",
            },
        ],
        description: "Echo/delay effect",
    },
];

// ============================================================================
// CHANNEL EFFECT PROCESSING
// ============================================================================

/// Applies channel effects to a mono sample and returns stereo (left, right)
///
/// This function takes a raw sample from the instrument and applies all
/// channel-level effects (tremolo, bitcrush, distortion, amplitude, pan)
pub fn apply_channel_effects(
    input_sample: f32,
    effects: &mut ChannelEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    let mut sample = input_sample;

    // ---- Chorus (if enabled) ----
    if effects.chorus_mix > 0.0 && effects.chorus_rate_hz > 0.0 {
        sample = apply_mono_chorus(sample, effects, sample_rate);
    }

    // ---- Tremolo (volume wobble) ----
    if effects.tremolo_rate_hz > 0.0 && effects.tremolo_depth > 0.0 {
        // Calculate LFO value (sine wave oscillating between 0 and 1)
        let lfo = effects.tremolo_phase.sin();

        // Apply tremolo: reduce volume by depth amount according to LFO
        // When LFO is at -1, we reduce by full depth
        // When LFO is at +1, we don't reduce at all
        let amplitude_modulation = 1.0 - effects.tremolo_depth * (1.0 - lfo) / 2.0;
        sample *= amplitude_modulation;

        // Advance tremolo LFO phase
        effects.tremolo_phase += TWO_PI * effects.tremolo_rate_hz / sample_rate as f32;
        if effects.tremolo_phase >= TWO_PI {
            effects.tremolo_phase -= TWO_PI;
        }
    }

    // ---- Bitcrush (bit depth reduction) ----
    if effects.bitcrush_bits < 16 {
        // Calculate how many quantization levels we have
        let quantization_levels = 2.0_f32.powi(effects.bitcrush_bits as i32);

        // Quantize the sample to the reduced bit depth
        sample = (sample * quantization_levels).round() / quantization_levels;
    }

    // ---- Distortion (soft clipping) ----
    if effects.distortion_amount > 0.0 {
        // Apply drive (amplification before clipping)
        let drive = 1.0 + effects.distortion_amount * 10.0;
        let driven_sample = sample * drive;

        // Soft clip using tanh-like curve
        // This creates harmonic overtones without harsh digital clipping
        sample = driven_sample / (1.0 + driven_sample.abs());
    }

    // ---- Amplitude (volume) ----
    sample *= effects.amplitude;

    // ---- Pan (stereo positioning) ----
    // Using constant-power panning (square root law)
    let pan_left_coefficient = ((1.0 - effects.pan) * 0.5).sqrt();
    let pan_right_coefficient = ((1.0 + effects.pan) * 0.5).sqrt();

    let left_sample = sample * pan_left_coefficient;
    let right_sample = sample * pan_right_coefficient;

    (left_sample, right_sample)
}

/// Calculates vibrato frequency multiplier
/// This is called separately from other effects because vibrato affects
/// the oscillator phase, not the final sample
///
/// Returns a multiplier to apply to the base frequency
pub fn calculate_vibrato_multiplier(effects: &mut ChannelEffectState, sample_rate: u32) -> f32 {
    if effects.vibrato_rate_hz > 0.0 && effects.vibrato_depth_semitones > 0.0 {
        // LFO oscillates between -1 and +1
        let lfo = effects.vibrato_phase.sin();

        // Convert semitone depth to frequency ratio
        // A semitone is a factor of 2^(1/12) = ~1.059
        let frequency_multiplier = 2.0_f32.powf(lfo * effects.vibrato_depth_semitones / 12.0);

        // Advance vibrato LFO phase
        effects.vibrato_phase += TWO_PI * effects.vibrato_rate_hz / sample_rate as f32;
        if effects.vibrato_phase >= TWO_PI {
            effects.vibrato_phase -= TWO_PI;
        }

        frequency_multiplier
    } else {
        1.0 // No vibrato - no frequency change
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

    // Calculate delay time modulated by LFO
    let base_delay_ms = 7.0; // Base delay (center of modulation)
    let lfo = effects.chorus_phase.sin();
    let modulated_delay_ms = base_delay_ms + lfo * effects.chorus_depth_ms;
    let delay_samples = (modulated_delay_ms / 1000.0 * sample_rate as f32).max(1.0);

    // Read from delay buffer with linear interpolation
    let delay_samples_int = delay_samples as usize;
    let delay_samples_frac = delay_samples - delay_samples_int as f32;

    let read_pos_1 = (effects.chorus_write_position + buffer_len - delay_samples_int) % buffer_len;
    let read_pos_2 = (read_pos_1 + buffer_len - 1) % buffer_len;

    let delayed_sample = lerp(
        effects.chorus_buffer[read_pos_1],
        effects.chorus_buffer[read_pos_2],
        delay_samples_frac,
    );

    // Write to buffer (with feedback)
    effects.chorus_buffer[effects.chorus_write_position] =
        input_sample + delayed_sample * effects.chorus_feedback;

    // Advance write position
    effects.chorus_write_position = (effects.chorus_write_position + 1) % buffer_len;

    // Advance LFO phase
    effects.chorus_phase += TWO_PI * effects.chorus_rate_hz / sample_rate as f32;
    if effects.chorus_phase >= TWO_PI {
        effects.chorus_phase -= TWO_PI;
    }

    // Mix dry and wet
    lerp(input_sample, delayed_sample, effects.chorus_mix)
}

// ============================================================================
// MASTER EFFECT PROCESSING
// ============================================================================

/// Applies all master effects to a stereo signal
///
/// Parameters:
/// - left: Left channel input
/// - right: Right channel input
/// - effects: Mutable reference to master effect state (contains buffers)
/// - sample_rate: Current sample rate
///
/// Returns: (processed_left, processed_right)
pub fn apply_master_effects(
    mut left: f32,
    mut right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    // ---- Reverb 1 (Simple) ----
    if effects.reverb1_enabled && effects.reverb1_mix > 0.001 {
        let (reverb_left, reverb_right) = apply_reverb1(left, right, effects, sample_rate);
        left = reverb_left;
        right = reverb_right;
    }

    // ---- Reverb 2 (Advanced) ----
    if effects.reverb2_enabled && effects.reverb2_mix > 0.001 {
        let (reverb_left, reverb_right) = apply_reverb2(left, right, effects, sample_rate);
        left = reverb_left;
        right = reverb_right;
    }

    // ---- Delay ----
    if effects.delay_enabled && effects.delay_feedback > 0.001 {
        let (delay_left, delay_right) = apply_delay(left, right, effects);
        left = delay_left;
        right = delay_right;
    }

    // ---- Chorus (Master) ----
    if effects.chorus_enabled && effects.chorus_mix > 0.001 {
        let (chorus_left, chorus_right) = apply_master_chorus(left, right, effects, sample_rate);
        left = chorus_left;
        right = chorus_right;
    }

    // ---- Master Amplitude ----
    left *= effects.amplitude;
    right *= effects.amplitude;

    // ---- Master Pan ----
    if effects.pan != 0.0 {
        let pan_left = ((1.0 - effects.pan) * 0.5).sqrt();
        let pan_right = ((1.0 + effects.pan) * 0.5).sqrt();
        left *= pan_left;
        right *= pan_right;
    }

    (left, right)
}

/// Reverb 1: Simple delay-based reverb (original implementation)
fn apply_reverb1(
    left: f32,
    right: f32,
    effects: &mut MasterEffectState,
    sample_rate: u32,
) -> (f32, f32) {
    if effects.reverb1_buffer.is_empty() {
        return (left, right);
    }

    // Calculate delay time based on room size
    let delay_samples = (effects.reverb1_room_size * sample_rate as f32 * 0.05) as usize;
    let delay_samples = delay_samples.min(effects.reverb1_buffer.len() - 1).max(1);

    // Read delayed sample
    let read_pos = (effects.reverb1_position + effects.reverb1_buffer.len() - delay_samples)
        % effects.reverb1_buffer.len();
    let reverb_sample = effects.reverb1_buffer[read_pos];

    // Write to buffer (mono mix of input + feedback)
    let mono_input = (left + right) * 0.5;
    effects.reverb1_buffer[effects.reverb1_position] = mono_input + reverb_sample * 0.5;

    // Advance position
    effects.reverb1_position = (effects.reverb1_position + 1) % effects.reverb1_buffer.len();

    // Mix wet and dry
    let wet = reverb_sample * effects.reverb1_mix;
    let dry = 1.0 - effects.reverb1_mix;

    (left * dry + wet, right * dry + wet)
}

/// Reverb 2: Advanced algorithmic reverb with comb filters and all-pass diffusion
///
/// This uses a Schroeder-style reverb architecture:
/// 1. Early reflections (short delays at prime-number intervals)
/// 2. Parallel comb filters (create the reverb tail density)
/// 3. Series all-pass filters (diffuse and smooth the reverb)
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

    // ---- Early Reflections ----
    // These create the initial sense of space before the main reverb tail
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

    // ---- Comb Filters (Parallel) ----
    // These create the dense reverb tail
    let mut comb_output = 0.0;

    // Calculate feedback based on decay time
    // Longer decay = higher feedback
    let target_decay_samples = effects.reverb2_decay * sample_rate as f32;

    for i in 0..effects.reverb2_comb_buffers.len() {
        if effects.reverb2_comb_buffers[i].is_empty() {
            continue;
        }

        let buffer_len = effects.reverb2_comb_buffers[i].len();
        let delay = ((buffer_len as f32 * room_scale) as usize).min(buffer_len - 1).max(1);

        // Read delayed sample
        let read_pos = (effects.reverb2_comb_positions[i] + buffer_len - delay) % buffer_len;
        let delayed = effects.reverb2_comb_buffers[i][read_pos];

        // Apply damping (low-pass filter on feedback)
        // This simulates air absorption of high frequencies
        effects.reverb2_comb_filters[i] = lerp(
            delayed,
            effects.reverb2_comb_filters[i],
            effects.reverb2_damping,
        );
        let filtered = effects.reverb2_comb_filters[i];

        // Calculate comb filter feedback to achieve desired decay time
        // feedback = 10^(-3 * delay_time / decay_time)
        let delay_time = delay as f32 / sample_rate as f32;
        let feedback = if target_decay_samples > 0.0 {
            10.0_f32.powf(-3.0 * delay_time / effects.reverb2_decay).min(0.98)
        } else {
            0.5
        };

        // Write to buffer
        let input_with_early = mono_input + early_reflections * 0.3;
        effects.reverb2_comb_buffers[i][effects.reverb2_comb_positions[i]] =
            input_with_early + filtered * feedback;

        effects.reverb2_comb_positions[i] = (effects.reverb2_comb_positions[i] + 1) % buffer_len;

        comb_output += delayed;
    }
    comb_output /= effects.reverb2_comb_buffers.len() as f32;

    // ---- All-Pass Filters (Series) ----
    // These diffuse the reverb, making it smoother and more natural
    let mut allpass_output = comb_output;
    let allpass_gain = 0.5; // Standard all-pass gain

    for i in 0..effects.reverb2_allpass_buffers.len() {
        if effects.reverb2_allpass_buffers[i].is_empty() {
            continue;
        }

        let buffer_len = effects.reverb2_allpass_buffers[i].len();
        let read_pos = (effects.reverb2_allpass_positions[i] + buffer_len - (buffer_len - 1))
            % buffer_len;

        let delayed = effects.reverb2_allpass_buffers[i][read_pos];

        // All-pass filter formula:
        // output = -input + delayed
        // buffer[write] = input + delayed * gain
        let output = -allpass_output * allpass_gain + delayed;
        effects.reverb2_allpass_buffers[i][effects.reverb2_allpass_positions[i]] =
            allpass_output + delayed * allpass_gain;

        effects.reverb2_allpass_positions[i] =
            (effects.reverb2_allpass_positions[i] + 1) % buffer_len;

        allpass_output = output;
    }

    // Mix wet and dry
    let wet = allpass_output * effects.reverb2_mix;
    let dry = 1.0 - effects.reverb2_mix;

    // Soft clip to prevent overflow
    let out_left = soft_clip(left * dry + wet);
    let out_right = soft_clip(right * dry + wet);

    (out_left, out_right)
}

/// Apply stereo delay effect
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

    // Read delayed samples
    let read_pos = (effects.delay_write_position + buffer_len - delay_samples) % buffer_len;
    let delayed_left = effects.delay_buffer_left[read_pos];
    let delayed_right = effects.delay_buffer_right[read_pos];

    // Write to buffers (input + feedback)
    effects.delay_buffer_left[effects.delay_write_position] =
        left + delayed_left * effects.delay_feedback;
    effects.delay_buffer_right[effects.delay_write_position] =
        right + delayed_right * effects.delay_feedback;

    // Advance position
    effects.delay_write_position = (effects.delay_write_position + 1) % buffer_len;

    // Mix (delay is additive, not wet/dry)
    (left + delayed_left * 0.5, right + delayed_right * 0.5)
}

/// Apply master stereo chorus
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

    // Left channel LFO
    let lfo_left = effects.chorus_phase.sin();
    let modulated_delay_left = base_delay_ms + lfo_left * effects.chorus_depth_ms;
    let delay_samples_left = (modulated_delay_left / 1000.0 * sample_rate as f32).max(1.0);

    // Right channel LFO (offset phase for stereo spread)
    let lfo_right = (effects.chorus_phase + PI * effects.chorus_stereo_spread).sin();
    let modulated_delay_right = base_delay_ms + lfo_right * effects.chorus_depth_ms;
    let delay_samples_right = (modulated_delay_right / 1000.0 * sample_rate as f32).max(1.0);

    // Read with interpolation - left
    let delay_int_left = delay_samples_left as usize;
    let delay_frac_left = delay_samples_left - delay_int_left as f32;
    let read_pos_1_left = (effects.chorus_write_position + buffer_len - delay_int_left) % buffer_len;
    let read_pos_2_left = (read_pos_1_left + buffer_len - 1) % buffer_len;
    let delayed_left = lerp(
        effects.chorus_buffer_left[read_pos_1_left],
        effects.chorus_buffer_left[read_pos_2_left],
        delay_frac_left,
    );

    // Read with interpolation - right
    let delay_int_right = delay_samples_right as usize;
    let delay_frac_right = delay_samples_right - delay_int_right as f32;
    let read_pos_1_right = (effects.chorus_write_position + buffer_len - delay_int_right) % buffer_len;
    let read_pos_2_right = (read_pos_1_right + buffer_len - 1) % buffer_len;
    let delayed_right = lerp(
        effects.chorus_buffer_right[read_pos_1_right],
        effects.chorus_buffer_right[read_pos_2_right],
        delay_frac_right,
    );

    // Write to buffers
    effects.chorus_buffer_left[effects.chorus_write_position] = left;
    effects.chorus_buffer_right[effects.chorus_write_position] = right;

    // Advance position
    effects.chorus_write_position = (effects.chorus_write_position + 1) % buffer_len;

    // Advance LFO
    effects.chorus_phase += TWO_PI * effects.chorus_rate_hz / sample_rate as f32;
    if effects.chorus_phase >= TWO_PI {
        effects.chorus_phase -= TWO_PI;
    }

    // Mix
    (
        lerp(left, delayed_left, effects.chorus_mix),
        lerp(right, delayed_right, effects.chorus_mix),
    )
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Finds a channel effect by name (case-insensitive)
pub fn find_channel_effect_by_name(name: &str) -> Option<&'static EffectDefinition> {
    let name_lower = name.to_lowercase();
    CHANNEL_EFFECT_REGISTRY
        .iter()
        .find(|e| e.name.to_lowercase() == name_lower || e.short_name.to_lowercase() == name_lower)
}

/// Finds a master effect by name (case-insensitive)
pub fn find_master_effect_by_name(name: &str) -> Option<&'static EffectDefinition> {
    let name_lower = name.to_lowercase();
    MASTER_EFFECT_REGISTRY
        .iter()
        .find(|e| e.name.to_lowercase() == name_lower || e.short_name.to_lowercase() == name_lower)
}

/// Checks if an effect name is a master-only effect
pub fn is_master_effect(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    matches!(
        name_lower.as_str(),
        "rv" | "reverb" | "rv2" | "reverb2" | "dl" | "delay" | "cl" | "clear"
    )
}

// ============================================================================
// UNIT TESTS
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
    fn test_find_effect_by_name() {
        assert!(find_channel_effect_by_name("amplitude").is_some());
        assert!(find_channel_effect_by_name("a").is_some());
        assert!(find_master_effect_by_name("reverb").is_some());
        assert!(find_master_effect_by_name("rv").is_some());
    }
}

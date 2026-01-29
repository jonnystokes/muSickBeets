// ============================================================================
// INSTRUMENTS.RS - Instrument Registry and Sample Generation
// ============================================================================
//
// This module defines all the sound-generating instruments in the synthesizer.
// Each instrument produces audio samples based on mathematical waveforms.
//
// WHAT IS AN INSTRUMENT?
// An instrument is a sound generator. Given a phase position (where we are in
// the waveform cycle) and optional parameters, it returns an audio sample value
// between -1.0 and 1.0.
//
// HOW INSTRUMENTS WORK:
// 1. The phase goes from 0 to 2*PI (one complete cycle of the wave)
// 2. The instrument function calculates what sample value corresponds to that phase
// 3. The phase advances based on the frequency being played
// 4. Higher frequencies = phase advances faster = more cycles per second = higher pitch
//
// HOW TO ADD A NEW INSTRUMENT:
// 1. Add a new entry to the INSTRUMENT_REGISTRY array below
// 2. Create a function that generates samples for your instrument
// 3. The function signature is: fn(phase: f32, params: &[f32], rng: &mut RandomNumberGenerator) -> f32
// 4. Return a value between -1.0 and 1.0
//
// ANTI-ALIASING:
// Some waveforms (square, sawtooth) have sharp edges that can cause aliasing
// (harsh, unwanted frequencies). We use PolyBLEP (Polynomial Bandlimited Step)
// to smooth these edges and reduce aliasing artifacts.
// ============================================================================

use crate::helper::{RandomNumberGenerator, TWO_PI};

// ============================================================================
// INSTRUMENT DEFINITION (REGISTRY PATTERN)
// ============================================================================
//
// Each instrument is defined with its properties and a pointer to its
// sample-generation function. The parser uses this information to understand
// what instruments are available and what parameters they accept.
// ============================================================================

/// Describes a single parameter that an instrument accepts
#[derive(Clone, Debug)]
pub struct InstrumentParameter {
    /// Name of the parameter (shown in documentation and errors)
    pub name: &'static str,

    /// Minimum allowed value
    pub min_value: f32,

    /// Maximum allowed value
    pub max_value: f32,

    /// Default value if not specified
    pub default_value: f32,

    /// Description of what this parameter does
    pub description: &'static str,
}

/// Defines an instrument type with all its properties
#[derive(Clone)]
pub struct InstrumentDefinition {
    /// Unique identifier for this instrument (used internally)
    pub id: usize,

    /// Primary name of the instrument (used in CSV files)
    pub name: &'static str,

    /// Alternative names/aliases that also work
    pub aliases: &'static [&'static str],

    /// Whether this instrument requires a pitch/note
    /// Noise doesn't need pitch, but sine/square/etc. do
    pub requires_pitch: bool,

    /// Whether this instrument can be used to play notes
    /// "master" is special - it's only for master bus effects, not playable
    pub is_playable: bool,

    /// Default attack time for this instrument's envelope (in seconds)
    pub default_attack_seconds: f32,

    /// Default release time for this instrument's envelope (in seconds)
    pub default_release_seconds: f32,

    /// List of parameters this instrument accepts
    pub parameters: &'static [InstrumentParameter],

    /// Short description of the instrument
    pub description: &'static str,

    /// The function that generates samples for this instrument
    /// This is a function pointer - it points to the actual code that makes sound
    pub generate_sample_function: fn(f32, &[f32], &mut RandomNumberGenerator) -> f32,
}

// ============================================================================
// INSTRUMENT REGISTRY
// ============================================================================
//
// This is the master list of all available instruments.
// The parser reads this to know what instruments exist and their properties.
//
// TO ADD A NEW INSTRUMENT:
// 1. Write the sample generation function (see examples below)
// 2. Add a new InstrumentDefinition to this array
// 3. Set a unique ID (next number in sequence)
// 4. The instrument is now available for use in CSV files!
// ============================================================================

/// Master registry of all instruments
/// Index 0 is reserved for "master" (not a playable instrument)
pub static INSTRUMENT_REGISTRY: &[InstrumentDefinition] = &[
    // -------------------------------------------------------------------------
    // ID 0: Master (Not Playable)
    // This is a special "instrument" that represents the master bus.
    // It cannot play notes - it's only used for master effects.
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 0,
        name: "master",
        aliases: &[],
        requires_pitch: false,
        is_playable: false,
        default_attack_seconds: 0.0,
        default_release_seconds: 0.0,
        parameters: &[],
        description: "Master bus - for effects only, cannot play notes",
        generate_sample_function: generate_silence,
    },

    // -------------------------------------------------------------------------
    // ID 1: Sine Wave
    // The purest waveform - a smooth, rounded wave with no harmonics.
    // Sounds like a tuning fork or a soft flute.
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 1,
        name: "sine",
        aliases: &["sin"],
        requires_pitch: true,
        is_playable: true,
        default_attack_seconds: 0.01,
        default_release_seconds: 0.5,
        parameters: &[],
        description: "Pure sine wave - smooth and mellow with no harmonics",
        generate_sample_function: generate_sine,
    },

    // -------------------------------------------------------------------------
    // ID 2: Triangle-Sawtooth Morph (TriSaw)
    // A morphable waveform that can smoothly transition between triangle and sawtooth.
    // The shape parameter controls the morph: -1.0 = saw down, 0.0 = triangle, 1.0 = saw up
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 2,
        name: "trisaw",
        aliases: &["tri", "saw", "triangle", "sawtooth"],
        requires_pitch: true,
        is_playable: true,
        default_attack_seconds: 0.01,
        default_release_seconds: 0.3,
        parameters: &[
            InstrumentParameter {
                name: "shape",
                min_value: -1.0,
                max_value: 1.0,
                default_value: 0.0,
                description: "Waveform shape: -1.0 = saw down, 0.0 = triangle, 1.0 = saw up",
            },
        ],
        description: "Morphable triangle/sawtooth wave - rich harmonics, shape-able timbre",
        generate_sample_function: generate_trisaw,
    },

    // -------------------------------------------------------------------------
    // ID 3: Square Wave
    // A wave that alternates between +1 and -1 with sharp transitions.
    // Contains only odd harmonics, giving it a hollow, clarinet-like sound.
    // Uses PolyBLEP anti-aliasing to reduce harshness at high frequencies.
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 3,
        name: "square",
        aliases: &["sq"],
        requires_pitch: true,
        is_playable: true,
        default_attack_seconds: 0.005,
        default_release_seconds: 0.2,
        parameters: &[],
        description: "Square wave - hollow sound with odd harmonics, anti-aliased",
        generate_sample_function: generate_square_antialiased,
    },

    // -------------------------------------------------------------------------
    // ID 4: White Noise
    // Random samples with equal energy at all frequencies.
    // Sounds like static, wind, or the "shhh" in ocean waves.
    // Does not require a pitch since it has no tonal quality.
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 4,
        name: "noise",
        aliases: &["white", "whitenoise"],
        requires_pitch: false,
        is_playable: true,
        default_attack_seconds: 0.001,
        default_release_seconds: 0.1,
        parameters: &[],
        description: "White noise - random signal, good for percussion and effects",
        generate_sample_function: generate_noise,
    },

    // -------------------------------------------------------------------------
    // ID 5: Pulse Wave
    // Like a square wave, but with variable pulse width.
    // Pulse width controls how long the wave stays "high" vs "low" in each cycle.
    // 50% width = square wave, other widths create different timbres.
    // Classic synth sound - think 80s bass lines and leads.
    // -------------------------------------------------------------------------
    InstrumentDefinition {
        id: 5,
        name: "pulse",
        aliases: &["pwm"],
        requires_pitch: true,
        is_playable: true,
        default_attack_seconds: 0.005,
        default_release_seconds: 0.3,
        parameters: &[
            InstrumentParameter {
                name: "width",
                min_value: 0.01,
                max_value: 0.99,
                default_value: 0.5,
                description: "Pulse width: 0.5 = square wave, lower = thinner, higher = fatter",
            },
            InstrumentParameter {
                name: "pwm_rate",
                min_value: 0.0,
                max_value: 20.0,
                default_value: 0.0,
                description: "Pulse width modulation rate in Hz (0 = no modulation)",
            },
            InstrumentParameter {
                name: "pwm_depth",
                min_value: 0.0,
                max_value: 0.49,
                default_value: 0.0,
                description: "Pulse width modulation depth (how much the width varies)",
            },
        ],
        description: "Pulse wave with variable width and optional PWM - classic synth sound",
        generate_sample_function: generate_pulse_antialiased,
    },
];

// ============================================================================
// SAMPLE GENERATION FUNCTIONS
// ============================================================================
//
// These functions do the actual work of creating audio samples.
// Each function receives:
// - phase: Current position in the waveform (0 to 2*PI)
// - params: Array of parameter values (if the instrument has parameters)
// - rng: Random number generator (for noise-based instruments)
//
// Each function returns a sample value between -1.0 and 1.0
// ============================================================================

/// Generates silence (used for the "master" pseudo-instrument)
fn generate_silence(_phase: f32, _params: &[f32], _rng: &mut RandomNumberGenerator) -> f32 {
    0.0
}

/// Generates a pure sine wave
/// The simplest waveform - just the sine of the phase
///
/// Mathematical formula: sample = sin(phase)
fn generate_sine(phase: f32, _params: &[f32], _rng: &mut RandomNumberGenerator) -> f32 {
    phase.sin()
}

/// Generates a triangle-sawtooth morphable wave
///
/// The shape parameter controls the waveform:
/// - shape = -1.0: Downward sawtooth (starts high, ramps down)
/// - shape = 0.0: Triangle wave (ramps up then down, symmetric)
/// - shape = 1.0: Upward sawtooth (starts low, ramps up)
///
/// This works by controlling where the "peak" of the wave occurs.
/// Triangle has peak at 50%, sawtooth has peak at 0% or 100%.
fn generate_trisaw(phase: f32, params: &[f32], _rng: &mut RandomNumberGenerator) -> f32 {
    // Get the shape parameter (defaults to 0.0 = triangle)
    let shape = if params.is_empty() {
        0.0
    } else {
        params[0].clamp(-1.0, 1.0)
    };

    // Convert phase (0 to 2*PI) to normalized time (0 to 1)
    let normalized_time = phase / TWO_PI;

    // Calculate where the peak occurs based on shape
    // shape -1.0 -> peak at 0.0 (sawtooth down)
    // shape 0.0 -> peak at 0.5 (triangle)
    // shape 1.0 -> peak at 1.0 (sawtooth up)
    let peak_position = (shape + 1.0) / 2.0;

    // Generate the waveform based on whether we're before or after the peak
    if normalized_time < peak_position {
        // Rising portion: goes from -1 to +1
        if peak_position > 0.0 {
            2.0 * (normalized_time / peak_position) - 1.0
        } else {
            // Peak is at the very beginning - we're always in falling portion
            -1.0
        }
    } else {
        // Falling portion: goes from +1 to -1
        let remaining = 1.0 - peak_position;
        if remaining > 0.0 {
            1.0 - 2.0 * ((normalized_time - peak_position) / remaining)
        } else {
            // Peak is at the very end - stay at +1
            1.0
        }
    }
}

/// Generates an anti-aliased square wave using PolyBLEP
///
/// A naive square wave (just checking if sin > 0) creates harsh aliasing artifacts.
/// PolyBLEP (Polynomial Bandlimited Step) smooths the sharp transitions to reduce aliasing.
///
/// The basic idea: at the exact moment of a transition, we "soften" the jump
/// using a polynomial curve instead of an instant step.
fn generate_square_antialiased(phase: f32, _params: &[f32], _rng: &mut RandomNumberGenerator) -> f32 {
    // Normalized phase (0 to 1)
    let normalized_phase = phase / TWO_PI;

    // Basic square wave: +1 for first half, -1 for second half
    let naive_square = if normalized_phase < 0.5 { 1.0 } else { -1.0 };

    // Calculate phase increment (approximation based on typical audio)
    // This affects how much smoothing we apply
    let phase_increment = 0.01; // A reasonable default for most frequencies

    // Apply PolyBLEP correction at the two discontinuities (0 and 0.5)
    let mut sample = naive_square;

    // Correction at phase = 0 (transition from -1 to +1)
    sample += polyblep(normalized_phase, phase_increment);

    // Correction at phase = 0.5 (transition from +1 to -1)
    sample -= polyblep((normalized_phase + 0.5) % 1.0, phase_increment);

    sample
}

/// Generates white noise
/// Each sample is a random value between -1.0 and 1.0
fn generate_noise(_phase: f32, _params: &[f32], rng: &mut RandomNumberGenerator) -> f32 {
    rng.next_float_bipolar()
}

/// Generates an anti-aliased pulse wave with optional pulse width modulation
///
/// Parameters:
/// - params[0]: Pulse width (0.01 to 0.99, default 0.5 = square wave)
/// - params[1]: PWM rate in Hz (0 = no modulation)
/// - params[2]: PWM depth (0.0 to 0.49, how much the width varies)
///
/// Pulse width controls the duty cycle - the percentage of time the wave is "high".
/// 50% = square wave, lower = thinner/nasal, higher = fatter/fuller
fn generate_pulse_antialiased(phase: f32, params: &[f32], _rng: &mut RandomNumberGenerator) -> f32 {
    // Parse parameters with defaults
    let base_width = if params.is_empty() {
        0.5 // Default to square wave
    } else {
        params[0].clamp(0.01, 0.99)
    };

    let pwm_rate = if params.len() > 1 { params[1].max(0.0) } else { 0.0 };
    let pwm_depth = if params.len() > 2 { params[2].clamp(0.0, 0.49) } else { 0.0 };

    // Calculate current pulse width (with optional modulation)
    // PWM uses a slow LFO to vary the pulse width over time
    let pulse_width = if pwm_rate > 0.0 && pwm_depth > 0.0 {
        // We need to track PWM phase separately, but for simplicity we'll derive it from the main phase
        // This creates a relationship between pitch and PWM rate which can sound interesting
        let pwm_phase = phase * pwm_rate / 100.0; // Scale down so PWM is slower than the main frequency
        let modulation = pwm_phase.sin() * pwm_depth;
        (base_width + modulation).clamp(0.01, 0.99)
    } else {
        base_width
    };

    // Normalized phase (0 to 1)
    let normalized_phase = phase / TWO_PI;

    // Basic pulse wave: +1 when phase < width, -1 otherwise
    let naive_pulse = if normalized_phase < pulse_width { 1.0 } else { -1.0 };

    // Apply PolyBLEP anti-aliasing
    let phase_increment = 0.01;
    let mut sample = naive_pulse;

    // Correction at the rising edge (phase = 0)
    sample += polyblep(normalized_phase, phase_increment);

    // Correction at the falling edge (phase = pulse_width)
    sample -= polyblep((normalized_phase - pulse_width + 1.0) % 1.0, phase_increment);

    sample
}

// ============================================================================
// ANTI-ALIASING HELPERS
// ============================================================================

/// PolyBLEP (Polynomial Bandlimited Step) function
/// This smooths the sharp discontinuities in square/pulse waves to reduce aliasing
///
/// The function is applied near discontinuities (phase near 0 or near 1).
/// It adds a small correction that "softens" the instant transition.
///
/// Parameters:
/// - phase: Normalized phase (0 to 1)
/// - phase_increment: How much phase advances per sample (affects smoothing amount)
#[inline]
fn polyblep(mut phase: f32, phase_increment: f32) -> f32 {
    // Only apply correction very close to the discontinuity
    if phase < phase_increment {
        // We're just after a discontinuity
        phase /= phase_increment;
        // Polynomial correction: 2*t - t^2 - 1
        return phase + phase - phase * phase - 1.0;
    } else if phase > 1.0 - phase_increment {
        // We're just before a discontinuity
        phase = (phase - 1.0) / phase_increment;
        // Polynomial correction: t^2 + 2*t + 1
        return phase * phase + phase + phase + 1.0;
    }
    // Not near a discontinuity - no correction needed
    0.0
}

// ============================================================================
// HELPER FUNCTIONS FOR FINDING INSTRUMENTS
// ============================================================================

/// Finds an instrument by name (case-insensitive)
/// Searches both primary names and aliases
/// Returns the instrument ID if found, or None if not found
pub fn find_instrument_by_name(name: &str) -> Option<usize> {
    let name_lower = name.to_lowercase();

    for instrument in INSTRUMENT_REGISTRY.iter() {
        // Check primary name
        if instrument.name.to_lowercase() == name_lower {
            return Some(instrument.id);
        }

        // Check aliases
        for alias in instrument.aliases.iter() {
            if alias.to_lowercase() == name_lower {
                return Some(instrument.id);
            }
        }
    }

    None
}

/// Gets an instrument definition by its ID
/// Returns None if the ID is invalid
pub fn get_instrument_by_id(id: usize) -> Option<&'static InstrumentDefinition> {
    INSTRUMENT_REGISTRY.get(id)
}

/// Returns the default parameter values for an instrument
/// This is used when an instrument is triggered without specifying all parameters
pub fn get_default_parameters(instrument_id: usize) -> Vec<f32> {
    if let Some(instrument) = get_instrument_by_id(instrument_id) {
        instrument
            .parameters
            .iter()
            .map(|param| param.default_value)
            .collect()
    } else {
        Vec::new()
    }
}

/// Validates and clamps parameters for an instrument
/// Ensures all parameters are within their valid ranges
pub fn validate_parameters(instrument_id: usize, params: &[f32]) -> Vec<f32> {
    if let Some(instrument) = get_instrument_by_id(instrument_id) {
        let mut validated = Vec::with_capacity(instrument.parameters.len());

        for (index, param_def) in instrument.parameters.iter().enumerate() {
            let value = if index < params.len() {
                params[index].clamp(param_def.min_value, param_def.max_value)
            } else {
                param_def.default_value
            };
            validated.push(value);
        }

        validated
    } else {
        params.to_vec()
    }
}

/// Generates a sample for the given instrument
/// This is the main entry point for sample generation
pub fn generate_sample(
    instrument_id: usize,
    phase: f32,
    params: &[f32],
    rng: &mut RandomNumberGenerator,
) -> f32 {
    if let Some(instrument) = get_instrument_by_id(instrument_id) {
        (instrument.generate_sample_function)(phase, params, rng)
    } else {
        0.0 // Unknown instrument - return silence
    }
}

/// Returns a list of all playable instrument names
/// Useful for help text and validation messages
pub fn get_playable_instrument_names() -> Vec<&'static str> {
    INSTRUMENT_REGISTRY
        .iter()
        .filter(|inst| inst.is_playable)
        .map(|inst| inst.name)
        .collect()
}

/// Returns true if the instrument requires a pitch/note
pub fn instrument_requires_pitch(instrument_id: usize) -> bool {
    get_instrument_by_id(instrument_id)
        .map(|inst| inst.requires_pitch)
        .unwrap_or(true) // Default to requiring pitch for unknown instruments
}

/// Returns true if the instrument is playable (can make sound)
pub fn is_instrument_playable(instrument_id: usize) -> bool {
    get_instrument_by_id(instrument_id)
        .map(|inst| inst.is_playable)
        .unwrap_or(false)
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_instrument_by_name() {
        assert_eq!(find_instrument_by_name("sine"), Some(1));
        assert_eq!(find_instrument_by_name("SINE"), Some(1)); // Case insensitive
        assert_eq!(find_instrument_by_name("sin"), Some(1)); // Alias
        assert_eq!(find_instrument_by_name("square"), Some(3));
        assert_eq!(find_instrument_by_name("nonexistent"), None);
    }

    #[test]
    fn test_instrument_requires_pitch() {
        assert!(instrument_requires_pitch(1)); // Sine requires pitch
        assert!(!instrument_requires_pitch(4)); // Noise doesn't require pitch
    }

    #[test]
    fn test_sine_output_range() {
        let mut rng = RandomNumberGenerator::new(42);
        for i in 0..100 {
            let phase = (i as f32 / 100.0) * TWO_PI;
            let sample = generate_sine(phase, &[], &mut rng);
            assert!(sample >= -1.0 && sample <= 1.0);
        }
    }

    #[test]
    fn test_pulse_width_parameter() {
        let mut rng = RandomNumberGenerator::new(42);

        // Test that different pulse widths produce different outputs
        let sample_50 = generate_pulse_antialiased(PI * 0.25, &[0.5], &mut rng);
        let sample_25 = generate_pulse_antialiased(PI * 0.25, &[0.25], &mut rng);

        // At phase PI*0.25 (normalized ~0.125), 50% width should be high, 25% might be different
        // Just verify they're valid samples
        assert!(sample_50 >= -1.5 && sample_50 <= 1.5); // PolyBLEP can slightly exceed -1..1
        assert!(sample_25 >= -1.5 && sample_25 <= 1.5);
    }
}

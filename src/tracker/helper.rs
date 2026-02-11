// ============================================================================
// HELPER.RS - Utility Functions and Shared Algorithms
// ============================================================================
//
// This module contains reusable helper functions used throughout the synthesizer.
// These are general-purpose utilities that don't belong to a specific category.
//
// CONTENTS:
// - Mathematical interpolation functions (lerp, smoothstep, etc.)
// - Pre-computed frequency lookup table for fast pitch-to-frequency conversion
// - Audio math utilities (decibels, panning, etc.)
// - Random number generation for noise synthesis
// - Common constants and conversion functions
//
// HOW TO ADD NEW HELPERS:
// 1. Add your function in the appropriate section below
// 2. Document it with a comment explaining what it does in plain English
// 3. If it's a math function, include the formula in the comment
// ============================================================================

// Re-export PI so other modules can use crate::helper::PI
pub use std::f32::consts::PI;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Two times PI, used frequently in oscillator calculations
/// This is the number of radians in a full circle (360 degrees)
pub const TWO_PI: f32 = 2.0 * PI;

/// The reference frequency for A4 (the A above middle C)
/// This is the standard tuning reference used worldwide
pub const A4_FREQUENCY_HZ: f32 = 440.0;

/// The MIDI note number for A4 (used as reference for calculations)
pub const A4_MIDI_NOTE: i32 = 69;

// ============================================================================
// PRE-COMPUTED FREQUENCY TABLE
// ============================================================================
//
// This table stores pre-calculated frequencies for all notes from octave 0 to 20.
// Using a lookup table is much faster than calculating frequencies on the fly,
// especially in the audio callback where speed is critical.
//
// The table is indexed by semitones from C0 (index 0).
// Each octave has 12 semitones, so:
// - C0 = index 0
// - C1 = index 12
// - C4 (middle C) = index 48
// - A4 = index 57 (440 Hz)
// ============================================================================

/// The lowest octave in our frequency table (octave 0)
pub const FREQUENCY_TABLE_MIN_OCTAVE: i32 = 0;

/// The highest octave in our frequency table (octave 20)
/// This goes way beyond human hearing but allows for extreme pitch effects
pub const FREQUENCY_TABLE_MAX_OCTAVE: i32 = 20;

/// Total number of semitones in our frequency table
/// 21 octaves * 12 semitones per octave = 252 entries
pub const FREQUENCY_TABLE_SIZE: usize = ((FREQUENCY_TABLE_MAX_OCTAVE - FREQUENCY_TABLE_MIN_OCTAVE + 1) * 12) as usize;

/// Pre-computed frequency table - generated once at startup
/// Each entry is the frequency in Hz for that semitone
/// Index 0 = C0, Index 12 = C1, Index 48 = C4, etc.
pub struct FrequencyTable {
    /// The actual frequency values in Hz
    frequencies: [f32; FREQUENCY_TABLE_SIZE],
}

impl FrequencyTable {
    /// Creates a new frequency table by calculating all frequencies
    /// This should be called once when the program starts
    ///
    /// The formula for frequency is: f = 440 * 2^((n - 69) / 12)
    /// where n is the MIDI note number and 69 is A4
    pub fn new() -> Self {
        let mut frequencies = [0.0_f32; FREQUENCY_TABLE_SIZE];

        for semitone_index in 0..FREQUENCY_TABLE_SIZE {
            // Convert table index to MIDI note number
            // C0 is MIDI note 12 (index 0 in our table)
            let midi_note = semitone_index as i32 + 12;

            // Calculate frequency using the standard formula
            // f = 440 * 2^((midi_note - 69) / 12)
            let semitones_from_a4 = midi_note - A4_MIDI_NOTE;
            let frequency = A4_FREQUENCY_HZ * 2.0_f32.powf(semitones_from_a4 as f32 / 12.0);

            frequencies[semitone_index] = frequency;
        }

        Self { frequencies }
    }

    /// Looks up the frequency for a given octave and semitone within that octave
    /// Returns None if the octave is outside our valid range
    ///
    /// Parameters:
    /// - octave: The octave number (0-20)
    /// - semitone: The semitone within the octave (0-11, where 0=C, 1=C#, etc.)
    pub fn get_frequency(&self, octave: i32, semitone: i32) -> Option<f32> {
        // Check if the octave is within our valid range
        if octave < FREQUENCY_TABLE_MIN_OCTAVE || octave > FREQUENCY_TABLE_MAX_OCTAVE {
            return None;
        }

        // Check if the semitone is valid (0-11)
        if semitone < 0 || semitone > 11 {
            return None;
        }

        // Calculate the index into our frequency table
        let table_index = ((octave - FREQUENCY_TABLE_MIN_OCTAVE) * 12 + semitone) as usize;

        // Make sure we don't go out of bounds
        if table_index >= FREQUENCY_TABLE_SIZE {
            return None;
        }

        Some(self.frequencies[table_index])
    }

    /// Looks up frequency by semitone offset from C0
    /// This is useful when you've already calculated the total semitone offset
    pub fn get_frequency_by_index(&self, semitone_index: usize) -> Option<f32> {
        if semitone_index >= FREQUENCY_TABLE_SIZE {
            return None;
        }
        Some(self.frequencies[semitone_index])
    }
}

// Implement Default so we can easily create a frequency table
impl Default for FrequencyTable {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// INTERPOLATION FUNCTIONS
// ============================================================================
//
// These functions are used to smoothly transition between values over time.
// They're essential for avoiding clicks/pops in audio and creating smooth
// parameter changes.
// ============================================================================

/// Linear interpolation between two values
/// This is the simplest form of interpolation - a straight line between a and b
///
/// Parameters:
/// - start_value: The starting value (returned when progress is 0.0)
/// - end_value: The ending value (returned when progress is 1.0)
/// - progress: How far along the transition (0.0 to 1.0)
///
/// Formula: result = start + (end - start) * progress
#[inline]
pub fn linear_interpolation(start_value: f32, end_value: f32, progress: f32) -> f32 {
    start_value + (end_value - start_value) * progress
}

/// Shorthand alias for linear_interpolation (commonly called "lerp")
#[inline]
pub fn lerp(start_value: f32, end_value: f32, progress: f32) -> f32 {
    linear_interpolation(start_value, end_value, progress)
}

/// Exponential interpolation - good for volume fades that sound natural to human ears
/// Human hearing is logarithmic, so exponential fades sound more "linear" to us
///
/// Parameters:
/// - start_value: The starting value (must be > 0 for pure exponential)
/// - end_value: The ending value
/// - progress: How far along the transition (0.0 to 1.0)
/// - curve_strength: How curved the interpolation is (1.0 = linear, higher = more curved)
#[inline]
pub fn exponential_interpolation(start_value: f32, end_value: f32, progress: f32, curve_strength: f32) -> f32 {
    let clamped_progress = progress.clamp(0.0, 1.0);

    // Apply exponential curve
    let curved_progress = clamped_progress.powf(curve_strength);

    start_value + (end_value - start_value) * curved_progress
}

/// Logarithmic interpolation - opposite of exponential, starts fast and slows down
/// Good for attack phases of envelopes
///
/// Parameters:
/// - start_value: The starting value
/// - end_value: The ending value
/// - progress: How far along the transition (0.0 to 1.0)
/// - curve_strength: How curved the interpolation is (1.0 = linear, higher = more curved)
#[inline]
pub fn logarithmic_interpolation(start_value: f32, end_value: f32, progress: f32, curve_strength: f32) -> f32 {
    let clamped_progress = progress.clamp(0.0, 1.0);

    // Apply logarithmic curve (inverse of exponential)
    let curved_progress = 1.0 - (1.0 - clamped_progress).powf(curve_strength);

    start_value + (end_value - start_value) * curved_progress
}

// ============================================================================
// AUDIO MATH UTILITIES
// ============================================================================
//
// These functions handle common audio calculations like converting between
// different units (decibels, linear amplitude, etc.) and audio-specific math.
// ============================================================================

// ============================================================================
// RANDOM NUMBER GENERATION
// ============================================================================
//
// These functions provide fast random number generation for noise synthesis.
// We use a Linear Congruential Generator (LCG) because it's extremely fast
// and the audio quality is good enough for noise synthesis.
// ============================================================================

/// Random number generator state
/// This holds the current state of the random number generator
/// Each channel should have its own state to avoid correlation between channels
#[derive(Clone, Debug)]
pub struct RandomNumberGenerator {
    /// The current state of the generator
    state: u32,
}

impl RandomNumberGenerator {
    /// Creates a new random number generator with the given seed
    /// Different seeds will produce different sequences of random numbers
    ///
    /// Parameters:
    /// - seed: Any number to initialize the generator (0 will be converted to 1)
    pub fn new(seed: u32) -> Self {
        // Ensure we don't start with 0 (which would cause all zeros)
        let initial_state = if seed == 0 { 1 } else { seed };
        Self { state: initial_state }
    }

    /// Creates a new generator seeded from a channel ID
    /// This ensures each channel has a unique random sequence
    pub fn from_channel_id(channel_id: usize) -> Self {
        // Use a hash-like transformation to get a good starting state
        let seed = (channel_id as u32)
            .wrapping_mul(1103515245)
            .wrapping_add(12345);
        Self::new(seed)
    }

    /// Generates the next random number in the sequence
    /// Returns a value between 0 and u32::MAX
    ///
    /// Uses the LCG formula: next = (a * current + c) mod m
    /// where a = 1103515245, c = 12345, m = 2^32
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        // LCG constants (same as glibc)
        self.state = self.state.wrapping_mul(1103515245).wrapping_add(12345);
        self.state
    }

    /// Generates a random floating point number between 0.0 and 1.0
    #[inline]
    pub fn next_float_0_to_1(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }

    /// Generates a random floating point number between -1.0 and 1.0
    /// This is the format needed for audio samples
    #[inline]
    pub fn next_float_bipolar(&mut self) -> f32 {
        self.next_float_0_to_1() * 2.0 - 1.0
    }
}

// ============================================================================
// PITCH PARSING UTILITIES
// ============================================================================
//
// These functions help convert note names (like "C4", "F#3") to frequencies.
// ============================================================================

/// Converts a note letter to its semitone offset from C
/// C=0, D=2, E=4, F=5, G=7, A=9, B=11
///
/// Returns None if the character is not a valid note letter
pub fn note_letter_to_semitone(note_char: char) -> Option<i32> {
    match note_char.to_ascii_lowercase() {
        'c' => Some(0),
        'd' => Some(2),
        'e' => Some(4),
        'f' => Some(5),
        'g' => Some(7),
        'a' => Some(9),
        'b' => Some(11),
        _ => None,
    }
}

/// Parses a pitch string like "C4", "F#3", "Bb5" and returns the frequency
/// This function uses the pre-computed frequency table for speed
///
/// Parameters:
/// - pitch_string: The note name (e.g., "C4", "f#3", "Bb5")
/// - frequency_table: Reference to the pre-computed frequency table
///
/// Returns: The frequency in Hz, or None if the pitch string is invalid
pub fn parse_pitch_to_frequency(pitch_string: &str, frequency_table: &FrequencyTable) -> Option<f32> {
    let pitch_lower = pitch_string.to_lowercase();
    let chars: Vec<char> = pitch_lower.chars().collect();

    if chars.is_empty() {
        return None;
    }

    // First character must be a note letter (a-g)
    let note_char = chars[0];
    let base_semitone = note_letter_to_semitone(note_char)?;

    let mut char_index = 1;

    // Check for sharp (#) or flat (b) modifier
    let mut semitone_offset = 0;
    if char_index < chars.len() {
        match chars[char_index] {
            '#' => {
                semitone_offset = 1;
                char_index += 1;
            }
            'b' => {
                // Make sure this 'b' is a flat modifier, not part of an octave number
                // If the next character is a digit, this 'b' could be ambiguous
                // But since we lowercased, 'b' followed by digit means flat
                if char_index + 1 < chars.len() || chars.len() == 2 {
                    semitone_offset = -1;
                    char_index += 1;
                }
            }
            _ => {}
        }
    }

    // Parse the octave number
    let octave_str: String = chars[char_index..].iter().collect();
    let octave: i32 = octave_str.parse().ok()?;

    // Check if the octave is within our valid range
    if octave < FREQUENCY_TABLE_MIN_OCTAVE || octave > FREQUENCY_TABLE_MAX_OCTAVE {
        return None;
    }

    // Calculate the total semitone within the octave
    let mut semitone_in_octave = base_semitone + semitone_offset;
    let mut adjusted_octave = octave;

    // Handle wrapping (e.g., Cb4 should be B3, B#4 should be C5)
    if semitone_in_octave < 0 {
        semitone_in_octave += 12;
        adjusted_octave -= 1;
    } else if semitone_in_octave >= 12 {
        semitone_in_octave -= 12;
        adjusted_octave += 1;
    }

    // Look up the frequency in the table
    frequency_table.get_frequency(adjusted_octave, semitone_in_octave)
}

// ============================================================================
// PHASE UTILITIES
// ============================================================================

/// Wraps a phase value to stay within 0 to 2*PI
/// This prevents the phase from growing infinitely large over time
#[inline]
pub fn wrap_phase(phase: f32) -> f32 {
    let mut wrapped = phase;
    while wrapped >= TWO_PI {
        wrapped -= TWO_PI;
    }
    while wrapped < 0.0 {
        wrapped += TWO_PI;
    }
    wrapped
}

/// Calculates the phase increment for a given frequency
/// This is how much the phase should advance per sample
#[inline]
pub fn calculate_phase_increment(frequency_hz: f32, sample_rate: u32) -> f32 {
    TWO_PI * frequency_hz / sample_rate as f32
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_table_a4() {
        let table = FrequencyTable::new();
        // A4 should be 440 Hz (octave 4, semitone 9)
        let freq = table.get_frequency(4, 9).unwrap();
        assert!((freq - 440.0).abs() < 0.01);
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
    }

    #[test]
    fn test_note_letter_to_semitone() {
        assert_eq!(note_letter_to_semitone('C'), Some(0));
        assert_eq!(note_letter_to_semitone('A'), Some(9));
        assert_eq!(note_letter_to_semitone('x'), None);
    }
}

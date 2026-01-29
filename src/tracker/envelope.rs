// ============================================================================
// ENVELOPE.RS - ADSR Envelope System with Registry Pattern
// ============================================================================
//
// This module provides amplitude envelopes that shape how sounds fade in and out.
// An envelope controls the volume of a sound over time, from the moment a note
// starts (attack) to when it fades away (release).
//
// WHAT IS AN ENVELOPE?
// When you press a key on a piano, the sound doesn't instantly appear at full
// volume - it has a brief "attack" phase. When you release the key, the sound
// gradually fades away - that's the "release" phase. An envelope models this
// behavior mathematically.
//
// ADSR STAGES:
// - Attack: How long it takes to reach maximum volume (0 to peak)
// - Decay: How long it takes to drop from peak to sustain level
// - Sustain: The volume level while the note is held (this is a level, not time)
// - Release: How long it takes to fade to silence after note-off
//
// HOW TO ADD A NEW ENVELOPE TYPE:
// 1. Add a new entry to the ENVELOPE_REGISTRY array below
// 2. Create a function that implements the envelope calculation
// 3. The function receives time position and parameters, returns amplitude 0.0-1.0
//
// CURVE TYPES:
// - Linear: Straight line from start to end (simple but can sound abrupt)
// - Exponential: Starts slow, speeds up (good for releases, sounds natural)
// - Logarithmic: Starts fast, slows down (good for attacks, sounds punchy)
// ============================================================================

use crate::helper::{lerp, exponential_interpolation, logarithmic_interpolation};

// ============================================================================
// ENVELOPE STATE
// ============================================================================
//
// This tracks which phase of the envelope we're currently in.
// The envelope moves through these phases in order: Attack -> Decay -> Sustain -> Release
// ============================================================================

/// The current phase of the envelope
/// Each phase has different behavior for calculating the amplitude
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopePhase {
    /// The envelope is not active - no sound should play
    Idle,

    /// Attack phase: amplitude rising from 0 to peak (1.0)
    /// This happens immediately when a note is triggered
    Attack,

    /// Decay phase: amplitude falling from peak (1.0) to sustain level
    /// This happens after attack completes
    Decay,

    /// Sustain phase: amplitude holds at sustain level
    /// This continues as long as the note is held
    Sustain,

    /// Release phase: amplitude falling from current level to 0
    /// This happens when the note is released
    Release,
}

// ============================================================================
// CURVE TYPE
// ============================================================================
//
// Different mathematical curves for how the envelope transitions between levels.
// The curve type affects how the sound "feels" - linear is robotic while
// exponential/logarithmic curves sound more natural to human ears.
// ============================================================================

/// The mathematical curve used for envelope transitions
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopeCurveType {
    /// Straight line from start to end
    /// Simple and predictable, but can sound mechanical
    Linear,

    /// Starts slow, then speeds up toward the end
    /// Good for release phases - sounds like natural decay
    /// The curve_strength parameter controls how curved it is
    Exponential,

    /// Starts fast, then slows down toward the end
    /// Good for attack phases - sounds punchy and responsive
    /// The curve_strength parameter controls how curved it is
    Logarithmic,
}

// ============================================================================
// ENVELOPE DEFINITION (REGISTRY PATTERN)
// ============================================================================
//
// Each envelope type is defined here with all its parameters.
// To add a new envelope, just add a new entry to the ENVELOPE_REGISTRY array.
// ============================================================================

/// Defines the parameters for an envelope type
/// This is the "blueprint" for how an envelope behaves
#[derive(Clone, Debug)]
pub struct EnvelopeDefinition {
    /// Unique identifier for this envelope type
    pub id: usize,

    /// Human-readable name for this envelope
    pub name: &'static str,

    /// Short description of what this envelope sounds like
    pub description: &'static str,

    /// Attack time in seconds (how long to reach peak volume)
    pub attack_time_seconds: f32,

    /// Decay time in seconds (how long to fall from peak to sustain)
    pub decay_time_seconds: f32,

    /// Sustain level from 0.0 to 1.0 (volume while note is held)
    /// Note: This is a LEVEL, not a time!
    pub sustain_level: f32,

    /// Release time in seconds (how long to fade to silence)
    pub release_time_seconds: f32,

    /// The curve type to use for the attack phase
    pub attack_curve: EnvelopeCurveType,

    /// The curve strength for attack (1.0 = linear, higher = more curved)
    pub attack_curve_strength: f32,

    /// The curve type to use for the decay phase
    pub decay_curve: EnvelopeCurveType,

    /// The curve strength for decay
    pub decay_curve_strength: f32,

    /// The curve type to use for the release phase
    pub release_curve: EnvelopeCurveType,

    /// The curve strength for release
    pub release_curve_strength: f32,
}

// ============================================================================
// ENVELOPE REGISTRY
// ============================================================================
//
// This is the master list of all available envelope types.
// Each entry defines a complete envelope with all its parameters.
//
// TO ADD A NEW ENVELOPE:
// 1. Copy one of the existing entries
// 2. Give it a new unique ID (next number in sequence)
// 3. Set a descriptive name
// 4. Adjust the timing and curve parameters to taste
// ============================================================================

/// The registry of all available envelope types
/// Index 0 is the default envelope used when no specific envelope is requested
pub static ENVELOPE_REGISTRY: &[EnvelopeDefinition] = &[
    // -------------------------------------------------------------------------
    // ID 0: Default Envelope
    // A natural-sounding envelope good for most instruments
    // Quick attack, slight dip to sustain, 2-second release
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 0,
        name: "default",
        description: "Natural envelope with quick attack and smooth 2-second release",
        attack_time_seconds: 0.01,           // 10ms attack - very quick, barely noticeable
        decay_time_seconds: 0.1,             // 100ms decay to sustain level
        sustain_level: 0.85,                 // Slight dip to 85% during sustain
        release_time_seconds: 2.0,           // 2 second fade out - smooth and gradual
        attack_curve: EnvelopeCurveType::Logarithmic,  // Fast start for punchy attack
        attack_curve_strength: 2.0,
        decay_curve: EnvelopeCurveType::Exponential,   // Natural decay curve
        decay_curve_strength: 1.5,
        release_curve: EnvelopeCurveType::Exponential, // Natural release sounds best
        release_curve_strength: 2.0,                    // Moderate curve for natural sound
    },

    // -------------------------------------------------------------------------
    // ID 1: Pluck Envelope
    // Sharp attack with quick decay - good for plucked string sounds
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 1,
        name: "pluck",
        description: "Sharp attack with fast decay, like a plucked string",
        attack_time_seconds: 0.005,          // 5ms - very snappy
        decay_time_seconds: 0.3,             // 300ms decay
        sustain_level: 0.3,                  // Low sustain for plucky sound
        release_time_seconds: 0.5,           // 500ms release
        attack_curve: EnvelopeCurveType::Linear,
        attack_curve_strength: 1.0,
        decay_curve: EnvelopeCurveType::Exponential,
        decay_curve_strength: 3.0,           // Strong curve for natural pluck decay
        release_curve: EnvelopeCurveType::Exponential,
        release_curve_strength: 2.0,
    },

    // -------------------------------------------------------------------------
    // ID 2: Pad Envelope
    // Slow attack and release - good for ambient pads and strings
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 2,
        name: "pad",
        description: "Slow attack and release for ambient pads and strings",
        attack_time_seconds: 0.5,            // 500ms - slow fade in
        decay_time_seconds: 0.2,             // 200ms slight decay
        sustain_level: 0.9,                  // High sustain
        release_time_seconds: 3.0,           // 3 second fade out
        attack_curve: EnvelopeCurveType::Logarithmic,
        attack_curve_strength: 1.5,
        decay_curve: EnvelopeCurveType::Linear,
        decay_curve_strength: 1.0,
        release_curve: EnvelopeCurveType::Exponential,
        release_curve_strength: 2.5,
    },

    // -------------------------------------------------------------------------
    // ID 3: Percussion Envelope
    // Instant attack, no sustain - good for drums and hits
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 3,
        name: "percussion",
        description: "Instant attack with no sustain, for drums and percussive sounds",
        attack_time_seconds: 0.001,          // 1ms - nearly instant
        decay_time_seconds: 0.0,             // No decay phase
        sustain_level: 1.0,                  // Full sustain (but release is fast)
        release_time_seconds: 0.1,           // 100ms release
        attack_curve: EnvelopeCurveType::Linear,
        attack_curve_strength: 1.0,
        decay_curve: EnvelopeCurveType::Linear,
        decay_curve_strength: 1.0,
        release_curve: EnvelopeCurveType::Exponential,
        release_curve_strength: 2.0,
    },

    // -------------------------------------------------------------------------
    // ID 4: Organ Envelope
    // Instant on/off like a real organ - no attack or release
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 4,
        name: "organ",
        description: "Instant on/off like a classic organ",
        attack_time_seconds: 0.005,          // 5ms to avoid clicks
        decay_time_seconds: 0.0,             // No decay
        sustain_level: 1.0,                  // Full sustain
        release_time_seconds: 0.05,          // 50ms to avoid clicks
        attack_curve: EnvelopeCurveType::Linear,
        attack_curve_strength: 1.0,
        decay_curve: EnvelopeCurveType::Linear,
        decay_curve_strength: 1.0,
        release_curve: EnvelopeCurveType::Linear,
        release_curve_strength: 1.0,
    },

    // -------------------------------------------------------------------------
    // ID 5: Swell Envelope
    // Very slow attack - good for swelling strings or crescendos
    // -------------------------------------------------------------------------
    EnvelopeDefinition {
        id: 5,
        name: "swell",
        description: "Very slow attack for dramatic swells and crescendos",
        attack_time_seconds: 2.0,            // 2 second swell
        decay_time_seconds: 0.0,             // No decay
        sustain_level: 1.0,                  // Full sustain
        release_time_seconds: 2.0,           // 2 second fade
        attack_curve: EnvelopeCurveType::Logarithmic,
        attack_curve_strength: 1.2,
        decay_curve: EnvelopeCurveType::Linear,
        decay_curve_strength: 1.0,
        release_curve: EnvelopeCurveType::Exponential,
        release_curve_strength: 2.0,
    },
];

// ============================================================================
// ENVELOPE STATE MACHINE
// ============================================================================
//
// This struct tracks the current state of an active envelope.
// It handles the math of moving through phases and calculating amplitude.
// ============================================================================

/// Tracks the runtime state of an envelope for a single note
#[derive(Clone, Debug)]
pub struct EnvelopeState {
    /// Which envelope definition we're using (index into ENVELOPE_REGISTRY)
    pub envelope_id: usize,

    /// Current phase of the envelope
    pub current_phase: EnvelopePhase,

    /// Current amplitude level (0.0 to 1.0)
    pub current_amplitude: f32,

    /// How many samples we've been in the current phase
    pub phase_elapsed_samples: u64,

    /// Total samples for the current phase (calculated from time and sample rate)
    pub phase_total_samples: u64,

    /// The amplitude level when we started the current phase
    /// Used for calculating smooth transitions
    pub phase_start_amplitude: f32,

    /// The target amplitude for the current phase
    pub phase_target_amplitude: f32,

    /// The sample rate (needed for time calculations)
    pub sample_rate: u32,
}

impl EnvelopeState {
    /// Creates a new envelope state with the specified envelope type
    /// The envelope starts in the Idle phase until trigger() is called
    pub fn new(envelope_id: usize, sample_rate: u32) -> Self {
        Self {
            envelope_id: envelope_id.min(ENVELOPE_REGISTRY.len() - 1),
            current_phase: EnvelopePhase::Idle,
            current_amplitude: 0.0,
            phase_elapsed_samples: 0,
            phase_total_samples: 0,
            phase_start_amplitude: 0.0,
            phase_target_amplitude: 0.0,
            sample_rate,
        }
    }

    /// Creates an envelope state using the default envelope (ID 0)
    pub fn new_default(sample_rate: u32) -> Self {
        Self::new(0, sample_rate)
    }

    /// Gets the envelope definition for this envelope
    fn get_definition(&self) -> &'static EnvelopeDefinition {
        &ENVELOPE_REGISTRY[self.envelope_id]
    }

    /// Triggers the envelope - starts the attack phase
    /// Call this when a note starts playing
    pub fn trigger(&mut self) {
        let definition = self.get_definition();

        self.current_phase = EnvelopePhase::Attack;
        self.phase_elapsed_samples = 0;
        self.phase_start_amplitude = self.current_amplitude;
        self.phase_target_amplitude = 1.0; // Attack always goes to peak (1.0)

        // Calculate how many samples the attack phase will take
        self.phase_total_samples = (definition.attack_time_seconds * self.sample_rate as f32) as u64;

        // If attack time is 0, skip directly to decay or sustain
        if self.phase_total_samples == 0 {
            self.current_amplitude = 1.0;
            self.advance_to_decay();
        }
    }

    /// Releases the envelope - starts the release phase
    /// Call this when a note stops playing
    pub fn release(&mut self) {
        // Only start release if we're not already releasing or idle
        if self.current_phase == EnvelopePhase::Release || self.current_phase == EnvelopePhase::Idle {
            return;
        }

        let definition = self.get_definition();

        self.current_phase = EnvelopePhase::Release;
        self.phase_elapsed_samples = 0;
        self.phase_start_amplitude = self.current_amplitude;
        self.phase_target_amplitude = 0.0; // Release always goes to silence

        // Calculate release time in samples
        self.phase_total_samples = (definition.release_time_seconds * self.sample_rate as f32) as u64;

        // If release time is 0, go straight to idle
        if self.phase_total_samples == 0 {
            self.current_amplitude = 0.0;
            self.current_phase = EnvelopePhase::Idle;
        }
    }

    /// Releases the envelope with a custom release time
    /// Useful for fast releases to avoid pops
    pub fn release_with_time(&mut self, release_time_seconds: f32) {
        if self.current_phase == EnvelopePhase::Release || self.current_phase == EnvelopePhase::Idle {
            return;
        }

        self.current_phase = EnvelopePhase::Release;
        self.phase_elapsed_samples = 0;
        self.phase_start_amplitude = self.current_amplitude;
        self.phase_target_amplitude = 0.0;
        self.phase_total_samples = (release_time_seconds * self.sample_rate as f32) as u64;

        if self.phase_total_samples == 0 {
            self.current_amplitude = 0.0;
            self.current_phase = EnvelopePhase::Idle;
        }
    }

    /// Forces the envelope to sustain phase with full amplitude
    /// Useful when a sustain command is received
    pub fn force_sustain(&mut self) {
        if self.current_phase != EnvelopePhase::Idle {
            let definition = self.get_definition();
            self.current_phase = EnvelopePhase::Sustain;
            self.current_amplitude = definition.sustain_level;
        }
    }

    /// Advances from attack phase to decay phase
    fn advance_to_decay(&mut self) {
        let definition = self.get_definition();

        // Check if we have a decay phase (decay time > 0 and sustain < 1.0)
        if definition.decay_time_seconds > 0.0 && definition.sustain_level < 1.0 {
            self.current_phase = EnvelopePhase::Decay;
            self.phase_elapsed_samples = 0;
            self.phase_start_amplitude = 1.0; // Coming from peak
            self.phase_target_amplitude = definition.sustain_level;
            self.phase_total_samples = (definition.decay_time_seconds * self.sample_rate as f32) as u64;
        } else {
            // Skip decay, go straight to sustain
            self.advance_to_sustain();
        }
    }

    /// Advances from decay phase to sustain phase
    fn advance_to_sustain(&mut self) {
        let definition = self.get_definition();
        self.current_phase = EnvelopePhase::Sustain;
        self.current_amplitude = definition.sustain_level;
    }

    /// Processes one sample and returns the current amplitude
    /// Call this once per sample in the audio callback
    pub fn process_sample(&mut self) -> f32 {
        let definition = self.get_definition();

        match self.current_phase {
            EnvelopePhase::Idle => {
                self.current_amplitude = 0.0;
            }

            EnvelopePhase::Attack => {
                if self.phase_total_samples > 0 {
                    let progress = self.phase_elapsed_samples as f32 / self.phase_total_samples as f32;

                    // Apply the attack curve
                    self.current_amplitude = apply_curve(
                        self.phase_start_amplitude,
                        self.phase_target_amplitude,
                        progress,
                        definition.attack_curve,
                        definition.attack_curve_strength,
                    );

                    self.phase_elapsed_samples += 1;

                    // Check if attack phase is complete
                    if self.phase_elapsed_samples >= self.phase_total_samples {
                        self.current_amplitude = 1.0;
                        self.advance_to_decay();
                    }
                } else {
                    self.current_amplitude = 1.0;
                    self.advance_to_decay();
                }
            }

            EnvelopePhase::Decay => {
                if self.phase_total_samples > 0 {
                    let progress = self.phase_elapsed_samples as f32 / self.phase_total_samples as f32;

                    self.current_amplitude = apply_curve(
                        self.phase_start_amplitude,
                        self.phase_target_amplitude,
                        progress,
                        definition.decay_curve,
                        definition.decay_curve_strength,
                    );

                    self.phase_elapsed_samples += 1;

                    if self.phase_elapsed_samples >= self.phase_total_samples {
                        self.advance_to_sustain();
                    }
                } else {
                    self.advance_to_sustain();
                }
            }

            EnvelopePhase::Sustain => {
                // Sustain holds at the sustain level - no change over time
                self.current_amplitude = definition.sustain_level;
            }

            EnvelopePhase::Release => {
                if self.phase_total_samples > 0 {
                    let progress = self.phase_elapsed_samples as f32 / self.phase_total_samples as f32;

                    self.current_amplitude = apply_curve(
                        self.phase_start_amplitude,
                        self.phase_target_amplitude,
                        progress,
                        definition.release_curve,
                        definition.release_curve_strength,
                    );

                    self.phase_elapsed_samples += 1;

                    if self.phase_elapsed_samples >= self.phase_total_samples {
                        self.current_amplitude = 0.0;
                        self.current_phase = EnvelopePhase::Idle;
                    }
                } else {
                    self.current_amplitude = 0.0;
                    self.current_phase = EnvelopePhase::Idle;
                }
            }
        }

        self.current_amplitude
    }

    /// Returns true if the envelope has finished (is idle and amplitude is zero)
    pub fn is_finished(&self) -> bool {
        self.current_phase == EnvelopePhase::Idle && self.current_amplitude < 0.001
    }

    /// Returns true if the envelope is currently active (not idle)
    pub fn is_active(&self) -> bool {
        self.current_phase != EnvelopePhase::Idle
    }
}

// ============================================================================
// CURVE APPLICATION
// ============================================================================

/// Applies the specified curve type to interpolate between two values
///
/// Parameters:
/// - start_value: The starting amplitude
/// - end_value: The target amplitude
/// - progress: How far through the phase (0.0 to 1.0)
/// - curve_type: Which mathematical curve to use
/// - curve_strength: How "curved" the curve is (1.0 = linear equivalent)
fn apply_curve(
    start_value: f32,
    end_value: f32,
    progress: f32,
    curve_type: EnvelopeCurveType,
    curve_strength: f32,
) -> f32 {
    match curve_type {
        EnvelopeCurveType::Linear => {
            lerp(start_value, end_value, progress)
        }
        EnvelopeCurveType::Exponential => {
            exponential_interpolation(start_value, end_value, progress, curve_strength)
        }
        EnvelopeCurveType::Logarithmic => {
            logarithmic_interpolation(start_value, end_value, progress, curve_strength)
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Finds an envelope definition by name (case-insensitive)
/// Returns the envelope ID if found, or None if not found
pub fn find_envelope_by_name(name: &str) -> Option<usize> {
    let name_lower = name.to_lowercase();
    ENVELOPE_REGISTRY
        .iter()
        .find(|envelope| envelope.name.to_lowercase() == name_lower)
        .map(|envelope| envelope.id)
}

/// Gets the default envelope ID (always 0)
pub fn get_default_envelope_id() -> usize {
    0
}

/// Returns a list of all available envelope names
/// Useful for displaying options to users or for help text
pub fn get_all_envelope_names() -> Vec<&'static str> {
    ENVELOPE_REGISTRY.iter().map(|e| e.name).collect()
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_trigger_and_release() {
        let mut envelope = EnvelopeState::new_default(48000);

        // Should start idle
        assert_eq!(envelope.current_phase, EnvelopePhase::Idle);

        // Trigger should move to attack
        envelope.trigger();
        assert_eq!(envelope.current_phase, EnvelopePhase::Attack);

        // Process some samples
        for _ in 0..1000 {
            envelope.process_sample();
        }

        // Should have some amplitude now
        assert!(envelope.current_amplitude > 0.0);

        // Release should move to release phase
        envelope.release();
        assert_eq!(envelope.current_phase, EnvelopePhase::Release);
    }

    #[test]
    fn test_find_envelope_by_name() {
        assert_eq!(find_envelope_by_name("default"), Some(0));
        assert_eq!(find_envelope_by_name("pluck"), Some(1));
        assert_eq!(find_envelope_by_name("nonexistent"), None);
    }
}

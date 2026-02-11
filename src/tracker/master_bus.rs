// ============================================================================
// MASTER_BUS.RS - Master Output Bus and Global Effects
// ============================================================================
//
// This module handles the final stage of audio processing. After all channels
// are mixed together, the combined signal passes through the master bus where
// global effects like reverb and delay are applied.
//
// WHAT IS THE MASTER BUS?
// The master bus is like the final mixing console in a recording studio.
// All individual channels (voices) are combined here, and then global effects
// are applied that affect the entire mix:
// - Reverb (adds space/ambience)
// - Delay (echo effects)
// - Master amplitude (overall volume)
// - Master pan (stereo position of entire mix)
// - Chorus (adds width and richness to entire mix)
//
// SIGNAL FLOW:
// Channels → Mixer → Master Bus Effects → Output
//
// TRANSITIONS:
// Like channel effects, master effects can transition smoothly to avoid clicks.
// This allows for things like fading the entire mix to silence.
// ============================================================================

use crate::helper::lerp;
use crate::effects::{MasterEffectState, apply_master_effects};

// ============================================================================
// MASTER TRANSITION STATE
// ============================================================================
//
// Tracks smooth transitions of master bus parameters.
// This allows effects to change gradually instead of instantly.
// ============================================================================

/// Stores the starting values for a master bus transition
#[derive(Clone, Debug)]
pub struct MasterTransitionState {
    /// Starting amplitude
    pub amplitude: f32,

    /// Starting pan position
    pub pan: f32,

    /// Starting reverb 1 room size
    pub reverb1_room_size: f32,

    /// Starting reverb 1 mix
    pub reverb1_mix: f32,

    /// Starting reverb 1 enabled state
    pub reverb1_enabled: bool,

    /// Starting reverb 2 room size
    pub reverb2_room_size: f32,

    /// Starting reverb 2 decay
    pub reverb2_decay: f32,

    /// Starting reverb 2 damping
    pub reverb2_damping: f32,

    /// Starting reverb 2 mix
    pub reverb2_mix: f32,

    /// Starting reverb 2 enabled state
    pub reverb2_enabled: bool,

    /// Starting delay time in samples
    pub delay_time_samples: u32,

    /// Starting delay feedback
    pub delay_feedback: f32,

    /// Starting delay enabled state
    pub delay_enabled: bool,

    /// Starting chorus mix
    pub chorus_mix: f32,

    /// Starting chorus rate
    pub chorus_rate_hz: f32,

    /// Starting chorus enabled state
    pub chorus_enabled: bool,
}

impl MasterTransitionState {
    /// Creates a transition state from the current master effects
    pub fn from_master_effects(effects: &MasterEffectState) -> Self {
        Self {
            amplitude: effects.amplitude,
            pan: effects.pan,
            reverb1_room_size: effects.reverb1_room_size,
            reverb1_mix: effects.reverb1_mix,
            reverb1_enabled: effects.reverb1_enabled,
            reverb2_room_size: effects.reverb2_room_size,
            reverb2_decay: effects.reverb2_decay,
            reverb2_damping: effects.reverb2_damping,
            reverb2_mix: effects.reverb2_mix,
            reverb2_enabled: effects.reverb2_enabled,
            delay_time_samples: effects.delay_time_samples,
            delay_feedback: effects.delay_feedback,
            delay_enabled: effects.delay_enabled,
            chorus_mix: effects.chorus_mix,
            chorus_rate_hz: effects.chorus_rate_hz,
            chorus_enabled: effects.chorus_enabled,
        }
    }
}

// ============================================================================
// MASTER BUS
// ============================================================================

/// The master output bus - processes the mixed output of all channels
#[derive(Clone, Debug)]
pub struct MasterBus {
    /// Current master effect state (holds all effect parameters and buffers)
    pub effects: MasterEffectState,

    /// Sample rate for time calculations
    pub sample_rate: u32,

    /// Whether a transition is currently active
    pub transition_active: bool,

    /// Total samples in the current transition
    pub transition_duration_samples: u32,

    /// Samples elapsed in the current transition
    pub transition_elapsed_samples: u32,

    /// State at the start of the transition
    pub transition_start: MasterTransitionState,

    /// Target state for the transition
    pub transition_target: MasterTransitionState,
}

impl MasterBus {
    /// Creates a new master bus with the given sample rate
    pub fn new(sample_rate: u32) -> Self {
        let mut effects = MasterEffectState::new();
        effects.initialize_buffers(sample_rate);

        // Create default transition states
        let default_transition = MasterTransitionState::from_master_effects(&effects);

        Self {
            effects,
            sample_rate,
            transition_active: false,
            transition_duration_samples: 0,
            transition_elapsed_samples: 0,
            transition_start: default_transition.clone(),
            transition_target: default_transition,
        }
    }

    /// Processes a stereo sample pair through all master effects
    /// This is the main entry point called for each sample
    ///
    /// Parameters:
    /// - left: Left channel input (sum of all channel outputs)
    /// - right: Right channel input (sum of all channel outputs)
    ///
    /// Returns: (processed_left, processed_right)
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Update transition if one is active
        if self.transition_active {
            self.update_transition();
        }

        // Apply all master effects
        apply_master_effects(left, right, &mut self.effects, self.sample_rate)
    }

    /// Updates the master bus transition (called each sample)
    fn update_transition(&mut self) {
        self.transition_elapsed_samples += 1;

        // Calculate progress (0.0 to 1.0)
        let progress = if self.transition_duration_samples > 0 {
            (self.transition_elapsed_samples as f32 / self.transition_duration_samples as f32)
                .clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Interpolate all parameters
        self.effects.amplitude = lerp(
            self.transition_start.amplitude,
            self.transition_target.amplitude,
            progress,
        );

        self.effects.pan = lerp(
            self.transition_start.pan,
            self.transition_target.pan,
            progress,
        );

        self.effects.reverb1_room_size = lerp(
            self.transition_start.reverb1_room_size,
            self.transition_target.reverb1_room_size,
            progress,
        );

        self.effects.reverb1_mix = lerp(
            self.transition_start.reverb1_mix,
            self.transition_target.reverb1_mix,
            progress,
        );

        self.effects.reverb2_room_size = lerp(
            self.transition_start.reverb2_room_size,
            self.transition_target.reverb2_room_size,
            progress,
        );

        self.effects.reverb2_decay = lerp(
            self.transition_start.reverb2_decay,
            self.transition_target.reverb2_decay,
            progress,
        );

        self.effects.reverb2_damping = lerp(
            self.transition_start.reverb2_damping,
            self.transition_target.reverb2_damping,
            progress,
        );

        self.effects.reverb2_mix = lerp(
            self.transition_start.reverb2_mix,
            self.transition_target.reverb2_mix,
            progress,
        );

        self.effects.delay_time_samples = lerp(
            self.transition_start.delay_time_samples as f32,
            self.transition_target.delay_time_samples as f32,
            progress,
        ) as u32;

        self.effects.delay_feedback = lerp(
            self.transition_start.delay_feedback,
            self.transition_target.delay_feedback,
            progress,
        );

        self.effects.chorus_mix = lerp(
            self.transition_start.chorus_mix,
            self.transition_target.chorus_mix,
            progress,
        );

        self.effects.chorus_rate_hz = lerp(
            self.transition_start.chorus_rate_hz,
            self.transition_target.chorus_rate_hz,
            progress,
        );

        // Check if transition is complete
        if progress >= 1.0 {
            // Apply final enabled states (these don't interpolate)
            self.effects.reverb1_enabled = self.transition_target.reverb1_enabled;
            self.effects.reverb2_enabled = self.transition_target.reverb2_enabled;
            self.effects.delay_enabled = self.transition_target.delay_enabled;
            self.effects.chorus_enabled = self.transition_target.chorus_enabled;

            self.transition_active = false;
        }
    }

    /// Clears all master effects to their default values
    ///
    /// Parameters:
    /// - transition_seconds: How long to take for the transition (0 = instant)
    pub fn clear_effects(&mut self, transition_seconds: f32) {
        if transition_seconds > 0.0 {
            // Save current state as start
            self.transition_start = MasterTransitionState::from_master_effects(&self.effects);

            // Set target to defaults
            self.transition_target = MasterTransitionState {
                amplitude: 1.0,
                pan: 0.0,
                reverb1_room_size: 0.5,
                reverb1_mix: 0.0,
                reverb1_enabled: false,
                reverb2_room_size: 0.5,
                reverb2_decay: 2.0,
                reverb2_damping: 0.5,
                reverb2_mix: 0.0,
                reverb2_enabled: false,
                delay_time_samples: self.sample_rate / 4,
                delay_feedback: 0.0,
                delay_enabled: false,
                chorus_mix: 0.0,
                chorus_rate_hz: 1.0,
                chorus_enabled: false,
            };

            self.transition_active = true;
            self.transition_duration_samples = (transition_seconds * self.sample_rate as f32) as u32;
            self.transition_elapsed_samples = 0;
        } else {
            // Instant clear
            self.effects.amplitude = 1.0;
            self.effects.pan = 0.0;
            self.effects.reverb1_enabled = false;
            self.effects.reverb2_enabled = false;
            self.effects.delay_enabled = false;
            self.effects.chorus_enabled = false;
            self.transition_active = false;
        }
    }

    /// Applies a master effect
    ///
    /// Parameters:
    /// - effect_name: The name of the effect (e.g., "rv", "dl", "a", "p")
    /// - parameters: The effect parameters as floats
    /// - transition_seconds: How long to transition (0 = instant)
    pub fn apply_effect(
        &mut self,
        effect_name: &str,
        parameters: &[f32],
        transition_seconds: f32,
    ) {
        match effect_name.to_lowercase().as_str() {
            // ---- Amplitude ----
            "a" | "amplitude" => {
                if !parameters.is_empty() {
                    let new_amplitude = parameters[0].clamp(0.0, 1.0);
                    self.apply_with_transition(|target| {
                        target.amplitude = new_amplitude;
                    }, transition_seconds);
                }
            }

            // ---- Pan ----
            "p" | "pan" => {
                if !parameters.is_empty() {
                    let new_pan = parameters[0].clamp(-1.0, 1.0);
                    self.apply_with_transition(|target| {
                        target.pan = new_pan;
                    }, transition_seconds);
                }
            }

            // ---- Reverb 1 (Simple) ----
            "rv" | "reverb" => {
                if parameters.len() >= 2 {
                    let room_size = parameters[0].clamp(0.0, 1.0);
                    let mix = parameters[1].clamp(0.0, 1.0);

                    self.apply_with_transition(|target| {
                        target.reverb1_room_size = room_size;
                        target.reverb1_mix = mix;
                        target.reverb1_enabled = mix > 0.0;
                    }, transition_seconds);
                }
            }

            // ---- Reverb 2 (Advanced) ----
            "rv2" | "reverb2" => {
                // Parameters: room, decay, damping, mix, predelay
                let room_size = if !parameters.is_empty() {
                    parameters[0].clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let decay = if parameters.len() > 1 {
                    parameters[1].clamp(0.1, 10.0)
                } else {
                    2.0
                };
                let damping = if parameters.len() > 2 {
                    parameters[2].clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let mix = if parameters.len() > 3 {
                    parameters[3].clamp(0.0, 1.0)
                } else {
                    0.3
                };
                let predelay = if parameters.len() > 4 {
                    parameters[4].clamp(0.0, 100.0)
                } else {
                    20.0
                };

                self.apply_with_transition(|target| {
                    target.reverb2_room_size = room_size;
                    target.reverb2_decay = decay;
                    target.reverb2_damping = damping;
                    target.reverb2_mix = mix;
                    target.reverb2_enabled = mix > 0.0;
                }, transition_seconds);

                // Predelay is set directly (not transitioned)
                self.effects.reverb2_predelay_ms = predelay;
            }

            // ---- Delay ----
            "dl" | "delay" => {
                if parameters.len() >= 2 {
                    let delay_time_seconds = parameters[0].clamp(0.01, 2.0);
                    let feedback = parameters[1].clamp(0.0, 0.95);
                    let delay_samples = (delay_time_seconds * self.sample_rate as f32) as u32;

                    self.apply_with_transition(|target| {
                        target.delay_time_samples = delay_samples;
                        target.delay_feedback = feedback;
                        target.delay_enabled = feedback > 0.0;
                    }, transition_seconds);
                }
            }

            // ---- Chorus ----
            "ch" | "chorus" => {
                // Parameters: mix, rate, depth, stereo_spread
                let mix = if !parameters.is_empty() {
                    parameters[0].clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let rate = if parameters.len() > 1 {
                    parameters[1].clamp(0.1, 5.0)
                } else {
                    1.0
                };
                let depth = if parameters.len() > 2 {
                    parameters[2].clamp(0.5, 10.0)
                } else {
                    3.0
                };
                let spread = if parameters.len() > 3 {
                    parameters[3].clamp(0.0, 1.0)
                } else {
                    0.5
                };

                self.apply_with_transition(|target| {
                    target.chorus_mix = mix;
                    target.chorus_rate_hz = rate;
                    target.chorus_enabled = mix > 0.0;
                }, transition_seconds);

                // Set depth and spread directly
                self.effects.chorus_depth_ms = depth;
                self.effects.chorus_stereo_spread = spread;
            }

            _ => {
                // Unknown effect - ignore silently or could log warning
            }
        }
    }

    /// Helper function to apply a change with optional transition
    fn apply_with_transition<F>(&mut self, modify_target: F, transition_seconds: f32)
    where
        F: FnOnce(&mut MasterTransitionState),
    {
        if transition_seconds > 0.0 {
            // Set up transition if not already active
            if !self.transition_active {
                self.transition_start = MasterTransitionState::from_master_effects(&self.effects);
                self.transition_target = self.transition_start.clone();
            }

            // Modify the target
            modify_target(&mut self.transition_target);

            // Start transition
            self.transition_active = true;
            self.transition_duration_samples = (transition_seconds * self.sample_rate as f32) as u32;
            self.transition_elapsed_samples = 0;
        } else {
            // Instant change - modify target then apply immediately
            let mut immediate = MasterTransitionState::from_master_effects(&self.effects);
            modify_target(&mut immediate);

            // Apply directly to effects
            self.effects.amplitude = immediate.amplitude;
            self.effects.pan = immediate.pan;
            self.effects.reverb1_room_size = immediate.reverb1_room_size;
            self.effects.reverb1_mix = immediate.reverb1_mix;
            self.effects.reverb1_enabled = immediate.reverb1_enabled;
            self.effects.reverb2_room_size = immediate.reverb2_room_size;
            self.effects.reverb2_decay = immediate.reverb2_decay;
            self.effects.reverb2_damping = immediate.reverb2_damping;
            self.effects.reverb2_mix = immediate.reverb2_mix;
            self.effects.reverb2_enabled = immediate.reverb2_enabled;
            self.effects.delay_time_samples = immediate.delay_time_samples;
            self.effects.delay_feedback = immediate.delay_feedback;
            self.effects.delay_enabled = immediate.delay_enabled;
            self.effects.chorus_mix = immediate.chorus_mix;
            self.effects.chorus_rate_hz = immediate.chorus_rate_hz;
            self.effects.chorus_enabled = immediate.chorus_enabled;
        }
    }

}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_bus_creation() {
        let bus = MasterBus::new(48000);
        assert_eq!(bus.sample_rate, 48000);
        assert_eq!(bus.effects.amplitude, 1.0);
        assert_eq!(bus.effects.pan, 0.0);
    }

    #[test]
    fn test_master_bus_process() {
        let mut bus = MasterBus::new(48000);

        // Process some samples
        for _ in 0..100 {
            let (left, right) = bus.process(0.5, 0.5);
            assert!(left >= -2.0 && left <= 2.0);
            assert!(right >= -2.0 && right <= 2.0);
        }
    }

    #[test]
    fn test_master_amplitude_effect() {
        let mut bus = MasterBus::new(48000);

        bus.apply_effect("a", &[0.5], 0.0);
        assert_eq!(bus.effects.amplitude, 0.5);
    }

    #[test]
    fn test_master_clear() {
        let mut bus = MasterBus::new(48000);

        // Enable some effects
        bus.apply_effect("rv", &[0.5, 0.5], 0.0);
        assert!(bus.effects.reverb1_enabled);

        // Clear
        bus.clear_effects(0.0);
        assert!(!bus.effects.reverb1_enabled);
    }
}

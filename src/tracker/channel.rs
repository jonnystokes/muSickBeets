// ============================================================================
// CHANNEL.RS - Audio Channel and Per-Channel Synthesis
// ============================================================================
//
// This module defines the Channel struct, which represents a single voice in
// the synthesizer. Each channel can play one note at a time with its own
// instrument, envelope, and effects.
//
// WHAT IS A CHANNEL?
// Think of channels like tracks in a DAW or voices in a choir. Each channel
// is independent - it has its own note, its own sound, its own effects.
// The synth mixes all channels together to create the final output.
//
// CHANNEL FEATURES:
// - Plays a single instrument at a time
// - Has its own ADSR envelope
// - Has per-channel effects (vibrato, tremolo, distortion, etc.)
// - Supports pitch glides (smooth transition from one note to another)
// - Supports instrument crossfades (smoothly change from one instrument to another)
//
// LIFECYCLE OF A NOTE:
// 1. Trigger: Note starts playing, envelope enters Attack phase
// 2. Sustain: Note held, envelope at Sustain level
// 3. Release: Note released, envelope fades out
// 4. Idle: Envelope finished, channel silent until next trigger
// ============================================================================

use crate::helper::{lerp, RandomNumberGenerator, calculate_phase_increment, wrap_phase};
use crate::envelope::{EnvelopeState, EnvelopePhase};
use crate::effects::{ChannelEffectState, apply_channel_effects, calculate_vibrato_multiplier};
use crate::instruments::generate_sample;

// ============================================================================
// TRANSITION STATE
// ============================================================================
//
// When effects change, they don't jump instantly (which would cause clicks).
// Instead, they smoothly transition from current to target values.
// This struct tracks that transition.
// ============================================================================

/// Tracks a smooth transition of effect parameters
#[derive(Clone, Debug)]
pub struct EffectTransition {
    /// How many samples the transition takes
    pub duration_samples: u32,

    /// How many samples have elapsed
    pub elapsed_samples: u32,

    /// The effect state we started from
    pub start_state: ChannelEffectState,

    /// The effect state we're transitioning to
    pub target_state: ChannelEffectState,
}

impl EffectTransition {
    /// Creates a new effect transition
    pub fn new(
        duration_seconds: f32,
        sample_rate: u32,
        start_state: ChannelEffectState,
        target_state: ChannelEffectState,
    ) -> Self {
        Self {
            duration_samples: (duration_seconds * sample_rate as f32) as u32,
            elapsed_samples: 0,
            start_state,
            target_state,
        }
    }

    /// Calculates the current progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.duration_samples == 0 {
            return 1.0;
        }
        (self.elapsed_samples as f32 / self.duration_samples as f32).clamp(0.0, 1.0)
    }

    /// Returns true if the transition is complete
    pub fn is_complete(&self) -> bool {
        self.elapsed_samples >= self.duration_samples
    }
}

// ============================================================================
// PITCH SLIDE STATE
// ============================================================================
//
// Pitch slides (portamento/glide) smoothly change from one pitch to another.
// This creates expressive vocal-like pitch bends.
// ============================================================================

/// Tracks a pitch slide (glide/portamento)
#[derive(Clone, Debug)]
pub struct PitchSlide {
    /// Starting frequency in Hz
    pub start_frequency_hz: f32,

    /// Target frequency in Hz
    pub target_frequency_hz: f32,

    /// Duration of the slide in seconds
    pub duration_seconds: f32,

    /// How many seconds have elapsed
    pub elapsed_seconds: f32,
}

impl PitchSlide {
    /// Creates a new pitch slide
    pub fn new(start_hz: f32, target_hz: f32, duration_seconds: f32) -> Self {
        Self {
            start_frequency_hz: start_hz,
            target_frequency_hz: target_hz,
            duration_seconds,
            elapsed_seconds: 0.0,
        }
    }

    /// Calculates the current frequency based on elapsed time
    pub fn current_frequency(&self) -> f32 {
        if self.duration_seconds <= 0.0 {
            return self.target_frequency_hz;
        }
        let progress = (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0);
        lerp(self.start_frequency_hz, self.target_frequency_hz, progress)
    }

    /// Returns true if the slide is complete
    pub fn is_complete(&self) -> bool {
        self.elapsed_seconds >= self.duration_seconds
    }

    /// Advances the slide by one sample
    pub fn advance(&mut self, sample_rate: u32) {
        self.elapsed_seconds += 1.0 / sample_rate as f32;
    }
}

// ============================================================================
// INSTRUMENT CROSSFADE
// ============================================================================
//
// When transitioning between instruments, we crossfade to avoid clicks.
// The old instrument fades out while the new one fades in.
// ============================================================================

/// Tracks an instrument crossfade
#[derive(Clone, Debug)]
pub struct InstrumentCrossfade {
    /// ID of the instrument we're fading from
    pub from_instrument_id: usize,

    /// ID of the instrument we're fading to
    pub to_instrument_id: usize,

    /// Duration of the crossfade in seconds
    pub duration_seconds: f32,

    /// How many seconds have elapsed
    pub elapsed_seconds: f32,
}

impl InstrumentCrossfade {
    /// Creates a new instrument crossfade
    pub fn new(from_id: usize, to_id: usize, duration_seconds: f32) -> Self {
        Self {
            from_instrument_id: from_id,
            to_instrument_id: to_id,
            duration_seconds,
            elapsed_seconds: 0.0,
        }
    }

    /// Calculates the crossfade amounts (from_gain, to_gain)
    pub fn gains(&self) -> (f32, f32) {
        if self.duration_seconds <= 0.0 {
            return (0.0, 1.0);
        }
        let progress = (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0);

        // Use sqrt for equal-power crossfade (sounds smoother than linear)
        let from_gain = (1.0 - progress).sqrt();
        let to_gain = progress.sqrt();

        (from_gain, to_gain)
    }

    /// Advances the crossfade by one sample
    pub fn advance(&mut self, sample_rate: u32) {
        self.elapsed_seconds += 1.0 / sample_rate as f32;
    }
}

// ============================================================================
// CHANNEL
// ============================================================================
//
// The main Channel struct. Each instance represents one voice in the synth.
// ============================================================================

/// A single audio channel (voice) in the synthesizer
#[derive(Clone, Debug)]
pub struct Channel {
    /// Unique identifier for this channel (0, 1, 2, ...)
    pub channel_id: usize,

    /// Whether this channel is currently producing sound
    pub is_active: bool,

    /// Current frequency in Hz
    pub frequency_hz: f32,

    /// Current phase position in the waveform (0 to 2*PI)
    pub phase: f32,

    /// Currently playing instrument ID
    pub instrument_id: usize,

    /// Parameters for the current instrument (e.g., trisaw shape, pulse width)
    pub instrument_parameters: Vec<f32>,

    /// Envelope state (handles ADSR amplitude shaping)
    pub envelope: EnvelopeState,

    /// Per-channel effects state
    pub effects: ChannelEffectState,

    /// Optional effect transition in progress
    pub effect_transition: Option<EffectTransition>,

    /// Optional pitch slide in progress
    pub pitch_slide: Option<PitchSlide>,

    /// Optional instrument crossfade in progress
    pub crossfade: Option<InstrumentCrossfade>,

    /// Random number generator for noise-based instruments
    pub random_generator: RandomNumberGenerator,

    /// Sample rate (needed for time calculations)
    pub sample_rate: u32,

    /// Total samples processed (for debugging/timing)
    pub total_samples_processed: u64,
}

impl Channel {
    /// Creates a new channel with the specified ID and sample rate
    pub fn new(channel_id: usize, sample_rate: u32) -> Self {
        let mut effects = ChannelEffectState::default();
        effects.initialize_chorus_buffer(sample_rate);

        Self {
            channel_id,
            is_active: false,
            frequency_hz: 440.0, // Default to A4
            phase: 0.0,
            instrument_id: 1, // Default to sine
            instrument_parameters: Vec::new(),
            envelope: EnvelopeState::new_default(sample_rate),
            effects,
            effect_transition: None,
            pitch_slide: None,
            crossfade: None,
            random_generator: RandomNumberGenerator::from_channel_id(channel_id),
            sample_rate,
            total_samples_processed: 0,
        }
    }

    /// Triggers a new note on this channel
    ///
    /// Parameters:
    /// - frequency_hz: The pitch to play in Hz
    /// - instrument_id: Which instrument to use
    /// - instrument_parameters: Parameters for the instrument (e.g., trisaw shape)
    /// - new_effects: The effect settings for this note
    /// - transition_seconds: How long to transition (0 = instant)
    /// - clear_effects: Whether to reset effects to defaults first
    pub fn trigger_note(
        &mut self,
        frequency_hz: f32,
        instrument_id: usize,
        instrument_parameters: Vec<f32>,
        new_effects: ChannelEffectState,
        transition_seconds: f32,
        clear_effects: bool,
    ) {
        // Determine if this is a smooth transition or a fresh trigger
        let is_smooth_transition = transition_seconds > 0.0 && self.is_active;

        if is_smooth_transition {
            // ---- SMOOTH TRANSITION (glide to new note without retriggering) ----

            // Set up pitch slide from current to new frequency
            self.pitch_slide = Some(PitchSlide::new(
                self.frequency_hz,
                frequency_hz,
                transition_seconds,
            ));

            // Set up instrument crossfade if changing instruments
            if instrument_id != self.instrument_id {
                self.crossfade = Some(InstrumentCrossfade::new(
                    self.instrument_id,
                    instrument_id,
                    transition_seconds,
                ));
                self.instrument_id = instrument_id;
            }

            // Update instrument parameters if provided
            if !instrument_parameters.is_empty() {
                self.instrument_parameters = instrument_parameters;
            }

            // Keep the envelope running (don't retrigger attack)
            // This is what makes the glide sound smooth
        } else {
            // ---- FRESH TRIGGER (new note from scratch) ----

            self.is_active = true;
            self.frequency_hz = frequency_hz;
            self.instrument_id = instrument_id;
            self.instrument_parameters = instrument_parameters;
            self.phase = 0.0;
            self.total_samples_processed = 0;

            // Clear any in-progress slides/crossfades
            self.pitch_slide = None;
            self.crossfade = None;

            // Trigger the envelope (starts attack phase)
            self.envelope.trigger();
        }

        // ---- HANDLE EFFECTS ----
        self.setup_effect_transition(new_effects, transition_seconds, clear_effects);
    }

    /// Triggers a pitchless instrument (like noise)
    /// Same as trigger_note but uses a dummy frequency
    pub fn trigger_pitchless(
        &mut self,
        instrument_id: usize,
        instrument_parameters: Vec<f32>,
        new_effects: ChannelEffectState,
        transition_seconds: f32,
        clear_effects: bool,
    ) {
        // Use 440 Hz as dummy frequency (noise doesn't use it anyway)
        self.trigger_note(
            440.0,
            instrument_id,
            instrument_parameters,
            new_effects,
            transition_seconds,
            clear_effects,
        );
    }

    /// Sets up an effect transition
    fn setup_effect_transition(
        &mut self,
        new_effects: ChannelEffectState,
        transition_seconds: f32,
        clear_effects: bool,
    ) {
        // Determine what we're transitioning to
        let target_effects = if clear_effects {
            // Clear to defaults first, then apply any new settings
            let mut target = ChannelEffectState::default();
            target.initialize_chorus_buffer(self.sample_rate);
            merge_effects(&mut target, &new_effects);
            target
        } else {
            // Apply new effects on top of current
            let mut target = self.effects.clone();
            merge_effects(&mut target, &new_effects);
            target
        };

        if transition_seconds > 0.0 {
            // Smooth transition over time
            self.effect_transition = Some(EffectTransition::new(
                transition_seconds,
                self.sample_rate,
                self.effects.clone(),
                target_effects,
            ));
        } else {
            // Instant change
            self.effects = target_effects;
            self.effect_transition = None;
        }
    }

    /// Releases the note (starts the release phase of the envelope)
    pub fn release(&mut self, release_time_seconds: f32) {
        if self.is_active && self.envelope.current_phase != EnvelopePhase::Release {
            self.envelope.release_with_time(release_time_seconds);
        }
    }

    /// Updates effects without triggering a new note
    pub fn update_effects(
        &mut self,
        new_effects: ChannelEffectState,
        transition_seconds: f32,
        clear_effects: bool,
    ) {
        self.setup_effect_transition(new_effects, transition_seconds, clear_effects);
    }

    /// Forces the envelope to sustain (keeps the note playing at sustain level)
    pub fn force_sustain(&mut self) {
        if self.is_active {
            self.envelope.force_sustain();
        }
    }

    /// Renders one sample from this channel
    /// Returns (left_sample, right_sample) for stereo output
    pub fn render_sample(&mut self) -> (f32, f32) {
        // If channel is not active, return silence
        if !self.is_active {
            return (0.0, 0.0);
        }

        // ---- UPDATE EFFECT TRANSITION ----
        self.update_effect_transition();

        // ---- UPDATE PITCH SLIDE ----
        if let Some(ref mut slide) = self.pitch_slide {
            self.frequency_hz = slide.current_frequency();
            slide.advance(self.sample_rate);

            if slide.is_complete() {
                self.frequency_hz = slide.target_frequency_hz;
            }
        }
        // Clean up completed slide
        if self.pitch_slide.as_ref().map(|s| s.is_complete()).unwrap_or(false) {
            self.pitch_slide = None;
            self.crossfade = None; // Crossfade completes with slide
        }

        // ---- CALCULATE VIBRATO ----
        let vibrato_multiplier = calculate_vibrato_multiplier(&mut self.effects, self.sample_rate);
        let modulated_frequency = self.frequency_hz * vibrato_multiplier;

        // ---- ADVANCE PHASE ----
        let phase_increment = calculate_phase_increment(modulated_frequency, self.sample_rate);
        self.phase += phase_increment;
        self.phase = wrap_phase(self.phase);

        // ---- GENERATE SAMPLE ----
        let raw_sample = if let Some(ref mut crossfade) = self.crossfade {
            // We're crossfading between instruments
            let (from_gain, to_gain) = crossfade.gains();

            let sample_from = generate_sample(
                crossfade.from_instrument_id,
                self.phase,
                &self.instrument_parameters,
                &mut self.random_generator,
            );

            let sample_to = generate_sample(
                crossfade.to_instrument_id,
                self.phase,
                &self.instrument_parameters,
                &mut self.random_generator,
            );

            crossfade.advance(self.sample_rate);

            sample_from * from_gain + sample_to * to_gain
        } else {
            // Normal single-instrument playback
            generate_sample(
                self.instrument_id,
                self.phase,
                &self.instrument_parameters,
                &mut self.random_generator,
            )
        };

        // ---- APPLY ENVELOPE ----
        let envelope_amplitude = self.envelope.process_sample();
        let enveloped_sample = raw_sample * envelope_amplitude;

        // ---- APPLY CHANNEL EFFECTS ----
        let (left_sample, right_sample) = apply_channel_effects(
            enveloped_sample,
            &mut self.effects,
            self.sample_rate,
        );

        // ---- UPDATE STATE ----
        self.total_samples_processed += 1;

        // Check if we should deactivate (envelope finished)
        if self.envelope.is_finished() {
            self.is_active = false;
        }

        (left_sample, right_sample)
    }

    /// Updates the effect transition (interpolates between start and target)
    fn update_effect_transition(&mut self) {
        if let Some(ref mut transition) = self.effect_transition {
            transition.elapsed_samples += 1;
            let progress = transition.progress();

            // Interpolate all effect parameters
            self.effects.amplitude = lerp(
                transition.start_state.amplitude,
                transition.target_state.amplitude,
                progress,
            );
            self.effects.pan = lerp(
                transition.start_state.pan,
                transition.target_state.pan,
                progress,
            );
            self.effects.vibrato_rate_hz = lerp(
                transition.start_state.vibrato_rate_hz,
                transition.target_state.vibrato_rate_hz,
                progress,
            );
            self.effects.vibrato_depth_semitones = lerp(
                transition.start_state.vibrato_depth_semitones,
                transition.target_state.vibrato_depth_semitones,
                progress,
            );
            self.effects.tremolo_rate_hz = lerp(
                transition.start_state.tremolo_rate_hz,
                transition.target_state.tremolo_rate_hz,
                progress,
            );
            self.effects.tremolo_depth = lerp(
                transition.start_state.tremolo_depth,
                transition.target_state.tremolo_depth,
                progress,
            );
            self.effects.distortion_amount = lerp(
                transition.start_state.distortion_amount,
                transition.target_state.distortion_amount,
                progress,
            );
            self.effects.chorus_mix = lerp(
                transition.start_state.chorus_mix,
                transition.target_state.chorus_mix,
                progress,
            );
            self.effects.chorus_rate_hz = lerp(
                transition.start_state.chorus_rate_hz,
                transition.target_state.chorus_rate_hz,
                progress,
            );
            self.effects.chorus_depth_ms = lerp(
                transition.start_state.chorus_depth_ms,
                transition.target_state.chorus_depth_ms,
                progress,
            );

            // Bitcrush interpolates as float then rounds
            let bitcrush_float = lerp(
                transition.start_state.bitcrush_bits as f32,
                transition.target_state.bitcrush_bits as f32,
                progress,
            );
            self.effects.bitcrush_bits = bitcrush_float.round() as u8;

            // Check if transition is complete
            if transition.is_complete() {
                self.effects = transition.target_state.clone();
            }
        }

        // Clean up completed transition
        if self.effect_transition.as_ref().map(|t| t.is_complete()).unwrap_or(false) {
            self.effect_transition = None;
        }
    }

    /// Returns true if this channel is currently producing sound
    pub fn is_playing(&self) -> bool {
        self.is_active
    }

}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Merges new effect values into current, only updating non-default values
/// This allows partial effect updates (e.g., just changing amplitude)
fn merge_effects(current: &mut ChannelEffectState, new: &ChannelEffectState) {
    let default = ChannelEffectState::default();

    // Only update values that differ from default (meaning they were explicitly set)
    if new.amplitude != default.amplitude {
        current.amplitude = new.amplitude;
    }
    if new.pan != default.pan {
        current.pan = new.pan;
    }
    if new.vibrato_rate_hz != default.vibrato_rate_hz {
        current.vibrato_rate_hz = new.vibrato_rate_hz;
        current.vibrato_depth_semitones = new.vibrato_depth_semitones;
    }
    if new.tremolo_rate_hz != default.tremolo_rate_hz {
        current.tremolo_rate_hz = new.tremolo_rate_hz;
        current.tremolo_depth = new.tremolo_depth;
    }
    if new.bitcrush_bits != default.bitcrush_bits {
        current.bitcrush_bits = new.bitcrush_bits;
    }
    if new.distortion_amount != default.distortion_amount {
        current.distortion_amount = new.distortion_amount;
    }
    if new.chorus_mix != default.chorus_mix {
        current.chorus_mix = new.chorus_mix;
        current.chorus_rate_hz = new.chorus_rate_hz;
        current.chorus_depth_ms = new.chorus_depth_ms;
        current.chorus_feedback = new.chorus_feedback;
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new(0, 48000);
        assert_eq!(channel.channel_id, 0);
        assert!(!channel.is_active);
        assert_eq!(channel.sample_rate, 48000);
    }

    #[test]
    fn test_channel_trigger() {
        let mut channel = Channel::new(0, 48000);
        let effects = ChannelEffectState::default();

        channel.trigger_note(440.0, 1, vec![], effects, 0.0, false);

        assert!(channel.is_active);
        assert_eq!(channel.frequency_hz, 440.0);
        assert_eq!(channel.instrument_id, 1);
    }

    #[test]
    fn test_channel_render() {
        let mut channel = Channel::new(0, 48000);
        let effects = ChannelEffectState::default();

        channel.trigger_note(440.0, 1, vec![], effects, 0.0, false);

        // Render some samples
        for _ in 0..100 {
            let (left, right) = channel.render_sample();
            // Samples should be within valid range
            assert!(left >= -2.0 && left <= 2.0); // Allow some headroom for effects
            assert!(right >= -2.0 && right <= 2.0);
        }
    }
}

// ============================================================================
// ENGINE.RS - Playback Engine and Sequencer
// ============================================================================
//
// This module contains the core playback engine that drives the synthesizer.
// It manages the sequencer (advancing through song rows) and coordinates
// all channels and the master bus.
//
// WHAT DOES THE ENGINE DO?
// 1. Loads parsed song data
// 2. Advances through rows at the configured tick rate
// 3. Dispatches cell actions to channels and master bus
// 4. Mixes all channel outputs together
// 5. Passes the mix through the master bus
// 6. Outputs final audio samples
//
// TIMING:
// Each row in the CSV plays for TICK_DURATION_SEC seconds.
// At 48000 Hz sample rate and 0.25s per row, that's 12000 samples per row.
// The engine counts samples and advances to the next row when needed.
// ============================================================================

use crate::channel::Channel;
use crate::master_bus::MasterBus;
use crate::parser::{SongData, CellAction, DebugLevel};

// ============================================================================
// ENGINE CONFIGURATION
// ============================================================================

/// Configuration for the playback engine
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Sample rate in Hz (typically 48000)
    pub sample_rate: u32,

    /// Number of audio channels (voices)
    pub channel_count: usize,

    /// How long each row plays in seconds
    pub tick_duration_seconds: f32,

    /// Default attack time for new notes (seconds)
    pub default_attack_seconds: f32,

    /// Default release time for slow release (seconds)
    pub default_release_seconds: f32,

    /// Fast release time to avoid pops (seconds)
    pub fast_release_seconds: f32,

    /// Debug output level
    pub debug_level: DebugLevel,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channel_count: 12,
            tick_duration_seconds: 0.25,
            default_attack_seconds: 0.10,
            default_release_seconds: 2.0,
            fast_release_seconds: 0.05,
            debug_level: DebugLevel::Off,
        }
    }
}

// ============================================================================
// PLAYBACK ENGINE
// ============================================================================

/// The main playback engine - coordinates everything
pub struct PlaybackEngine {
    /// The parsed song data
    song: SongData,

    /// Engine configuration
    config: EngineConfig,

    /// Current row being played (0-indexed)
    current_row: usize,

    /// Samples played in the current row
    samples_in_current_row: u32,

    /// Samples per row (calculated from tick duration and sample rate)
    samples_per_row: u32,

    /// All audio channels
    channels: Vec<Channel>,

    /// The master output bus
    master_bus: MasterBus,

    /// Whether playback has finished
    playback_finished: bool,

    /// Total samples rendered (for statistics)
    total_samples_rendered: u64,
}

impl PlaybackEngine {
    /// Creates a new playback engine with the given song and configuration
    pub fn new(song: SongData, config: EngineConfig) -> Self {
        // Calculate samples per row
        let samples_per_row = (config.tick_duration_seconds * config.sample_rate as f32) as u32;

        // Create channels
        let channels: Vec<Channel> = (0..config.channel_count)
            .map(|id| Channel::new(id, config.sample_rate))
            .collect();

        // Create master bus
        let master_bus = MasterBus::new(config.sample_rate);

        if config.debug_level >= DebugLevel::Basic {
            println!(
                "[ENGINE] Initialized: {} channels, {} samples/row ({:.2}s/row), {} rows total",
                config.channel_count,
                samples_per_row,
                config.tick_duration_seconds,
                song.row_count()
            );
        }

        Self {
            song,
            config,
            current_row: 0,
            samples_in_current_row: 0,
            samples_per_row,
            channels,
            master_bus,
            playback_finished: false,
            total_samples_rendered: 0,
        }
    }

    /// Advances to the next row and dispatches actions
    fn advance_row(&mut self) {
        // Check if we've reached the end
        if self.current_row >= self.song.rows.len() {
            self.playback_finished = true;
            return;
        }

        // Debug output
        if self.config.debug_level >= DebugLevel::Verbose {
            if self.current_row < self.song.raw_lines.len() {
                println!("Row {}", self.current_row);
                println!("{}\n", self.song.raw_lines[self.current_row]);
            }
        }

        // Get the actions for this row (clone to avoid borrow issues)
        let row_actions = self.song.rows[self.current_row].clone();

        // Dispatch each action to its channel
        for (channel_index, action) in row_actions.iter().enumerate() {
            if channel_index >= self.channels.len() {
                break;
            }

            self.dispatch_action(channel_index, action);
        }

        // Move to next row
        self.current_row += 1;
        self.samples_in_current_row = 0;
    }

    /// Dispatches a cell action to the appropriate channel
    fn dispatch_action(&mut self, channel_index: usize, action: &CellAction) {
        match action {
            CellAction::TriggerNote {
                pitch: _,
                frequency_hz,
                instrument_id,
                instrument_parameters,
                effects,
                transition_seconds,
                clear_effects,
            } => {
                self.channels[channel_index].trigger_note(
                    *frequency_hz,
                    *instrument_id,
                    instrument_parameters.clone(),
                    effects.clone(),
                    *transition_seconds,
                    *clear_effects,
                );
            }

            CellAction::TriggerPitchless {
                instrument_id,
                instrument_parameters,
                effects,
                transition_seconds,
                clear_effects,
            } => {
                self.channels[channel_index].trigger_pitchless(
                    *instrument_id,
                    instrument_parameters.clone(),
                    effects.clone(),
                    *transition_seconds,
                    *clear_effects,
                );
            }

            CellAction::Sustain => {
                // Keep playing - just ensure envelope stays in sustain
                self.channels[channel_index].force_sustain();
            }

            CellAction::SustainWithEffects {
                effects,
                transition_seconds,
                clear_first,
            } => {
                // Sustain the note
                self.channels[channel_index].force_sustain();

                // Update effects
                self.channels[channel_index].update_effects(
                    effects.clone(),
                    *transition_seconds,
                    *clear_first,
                );
            }

            CellAction::FastRelease => {
                self.channels[channel_index].release(self.config.fast_release_seconds);
            }

            CellAction::SlowRelease => {
                self.channels[channel_index].release(self.config.default_release_seconds);
            }

            CellAction::ChangeEffects {
                effects,
                transition_seconds,
                clear_first,
            } => {
                self.channels[channel_index].update_effects(
                    effects.clone(),
                    *transition_seconds,
                    *clear_first,
                );
            }

            CellAction::MasterEffects {
                clear_first,
                transition_seconds,
                effects,
            } => {
                // Clear first if requested
                if *clear_first {
                    self.master_bus.clear_effects(*transition_seconds);
                }

                // Apply each effect
                for (effect_name, params) in effects {
                    self.master_bus.apply_effect(effect_name, params, *transition_seconds);
                }
            }
        }
    }

    /// Processes a frame of audio
    /// Fills the output buffer with stereo samples (interleaved L R L R ...)
    pub fn process_frame(&mut self, output: &mut [f32]) {
        // Process samples in pairs (stereo)
        for sample_pair in output.chunks_mut(2) {
            // Check if we need to advance to the next row
            if self.samples_in_current_row >= self.samples_per_row {
                self.advance_row();
            }

            // If playback is finished, output silence
            if self.playback_finished {
                sample_pair[0] = 0.0;
                sample_pair[1] = 0.0;
                continue;
            }

            // Mix all channels together
            let mut left_sum = 0.0;
            let mut right_sum = 0.0;

            for channel in &mut self.channels {
                if channel.is_playing() {
                    let (left, right) = channel.render_sample();
                    left_sum += left;
                    right_sum += right;
                }
            }

            // Process through master bus
            let (final_left, final_right) = self.master_bus.process(left_sum, right_sum);

            // Clamp to valid range to prevent clipping
            sample_pair[0] = final_left.clamp(-1.0, 1.0);
            sample_pair[1] = final_right.clamp(-1.0, 1.0);

            // Update counters
            self.samples_in_current_row += 1;
            self.total_samples_rendered += 1;
        }
    }

    /// Returns true if playback has finished
    pub fn is_finished(&self) -> bool {
        self.playback_finished
    }

    /// Returns the current playback position in seconds
    pub fn get_position_seconds(&self) -> f32 {
        self.total_samples_rendered as f32 / self.config.sample_rate as f32
    }

    /// Returns the total duration in seconds
    pub fn get_total_duration_seconds(&self) -> f32 {
        self.song.row_count() as f32 * self.config.tick_duration_seconds
    }

    /// Returns the current row number
    pub fn get_current_row(&self) -> usize {
        self.current_row
    }

    /// Returns the total number of rows
    pub fn get_total_rows(&self) -> usize {
        self.song.row_count()
    }

    /// Gets the sample rate
    pub fn get_sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Gets the channel count
    pub fn get_channel_count(&self) -> usize {
        self.config.channel_count
    }

    /// Resets playback to the beginning
    pub fn reset(&mut self) {
        self.current_row = 0;
        self.samples_in_current_row = 0;
        self.playback_finished = false;
        self.total_samples_rendered = 0;

        // Reset all channels
        for channel in &mut self.channels {
            *channel = Channel::new(channel.channel_id, self.config.sample_rate);
        }

        // Reset master bus
        self.master_bus = MasterBus::new(self.config.sample_rate);
    }

    /// Renders the entire song to a buffer
    /// Returns a Vec of stereo samples (interleaved L R L R ...)
    /// This is used for WAV export
    pub fn render_to_buffer(&mut self) -> Vec<f32> {
        // Calculate total samples needed
        let total_samples = (self.get_total_duration_seconds() * self.config.sample_rate as f32) as usize * 2;

        // Add extra time for release tails (2 seconds)
        let extra_samples = (2.0 * self.config.sample_rate as f32) as usize * 2;
        let total_with_tail = total_samples + extra_samples;

        let mut buffer = vec![0.0; total_with_tail];

        // Reset to beginning
        self.reset();

        // Render in chunks
        let chunk_size = 1024;
        for chunk in buffer.chunks_mut(chunk_size) {
            self.process_frame(chunk);
        }

        buffer
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_song, MissingCellBehavior};
    use crate::helper::FrequencyTable;

    #[test]
    fn test_engine_creation() {
        let frequency_table = FrequencyTable::new();
        let song_text = "Voice0\nc4 sine\n-\n.";
        let song = parse_song(
            song_text,
            &frequency_table,
            1,
            MissingCellBehavior::SlowRelease,
            DebugLevel::Off,
        );

        let config = EngineConfig::default();
        let engine = PlaybackEngine::new(song, config);

        assert_eq!(engine.get_current_row(), 0);
        assert!(!engine.is_finished());
    }

    #[test]
    fn test_engine_render() {
        let frequency_table = FrequencyTable::new();
        let song_text = "Voice0\nc4 sine\n.";
        let song = parse_song(
            song_text,
            &frequency_table,
            1,
            MissingCellBehavior::SlowRelease,
            DebugLevel::Off,
        );

        let config = EngineConfig::default();
        let mut engine = PlaybackEngine::new(song, config);

        // Render some samples
        let mut buffer = vec![0.0; 1000];
        engine.process_frame(&mut buffer);

        // Should have rendered something
        assert!(engine.total_samples_rendered > 0);
    }
}

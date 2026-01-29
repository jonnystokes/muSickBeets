// ============================================================================
// STATE.RS - Effect State and Parameter Structures
// ============================================================================
//
// This module contains the state structures that effects use to maintain
// their internal state between samples (buffers, phases, filter states, etc.)
// and their parameter structures (user-configurable values).
//
// ============================================================================

use std::collections::VecDeque;

// ============================================================================
// EFFECT PARAMETERS
// ============================================================================
//
// These structures hold the user-configurable parameters for each effect type.
// Parameters are set from the CSV file and can be interpolated during transitions.
// ============================================================================

/// Parameters for amplitude/gain effect
#[derive(Clone, Debug)]
pub struct AmplitudeParams {
    pub gain: f32,          // Linear gain (0.0 to 2.0+)
    pub gain_db: f32,       // Gain in decibels (-inf to +12)
}

impl Default for AmplitudeParams {
    fn default() -> Self {
        Self { gain: 1.0, gain_db: 0.0 }
    }
}

/// Parameters for stereo pan effect
#[derive(Clone, Debug)]
pub struct PanParams {
    pub position: f32,      // -1.0 (left) to 1.0 (right)
}

impl Default for PanParams {
    fn default() -> Self {
        Self { position: 0.0 }
    }
}

/// Parameters for vibrato effect
#[derive(Clone, Debug)]
pub struct VibratoParams {
    pub rate_hz: f32,           // LFO frequency (0.1 to 20 Hz typical)
    pub depth_semitones: f32,   // Pitch deviation in semitones
    pub depth_cents: f32,       // Pitch deviation in cents (1/100 semitone)
    pub waveform: LfoWaveform,  // LFO shape
}

impl Default for VibratoParams {
    fn default() -> Self {
        Self {
            rate_hz: 5.0,
            depth_semitones: 0.0,
            depth_cents: 0.0,
            waveform: LfoWaveform::Sine,
        }
    }
}

/// Parameters for tremolo effect
#[derive(Clone, Debug)]
pub struct TremoloParams {
    pub rate_hz: f32,       // LFO frequency
    pub depth: f32,         // Modulation depth (0.0 to 1.0)
    pub waveform: LfoWaveform,
}

impl Default for TremoloParams {
    fn default() -> Self {
        Self {
            rate_hz: 5.0,
            depth: 0.0,
            waveform: LfoWaveform::Sine,
        }
    }
}

/// Parameters for chorus effect
#[derive(Clone, Debug)]
pub struct ChorusParams {
    pub rate_hz: f32,       // Modulation rate
    pub depth_ms: f32,      // Delay modulation depth in ms
    pub mix: f32,           // Wet/dry mix
    pub feedback: f32,      // Feedback amount (0.0 to 0.9)
    pub voices: u8,         // Number of chorus voices (1-4)
    pub stereo_spread: f32, // Stereo widening (0.0 to 1.0)
}

impl Default for ChorusParams {
    fn default() -> Self {
        Self {
            rate_hz: 1.0,
            depth_ms: 3.0,
            mix: 0.5,
            feedback: 0.2,
            voices: 2,
            stereo_spread: 0.5,
        }
    }
}

/// Parameters for phaser effect
#[derive(Clone, Debug)]
pub struct PhaserParams {
    pub rate_hz: f32,       // Sweep rate
    pub depth: f32,         // Sweep depth (0.0 to 1.0)
    pub feedback: f32,      // Feedback (-0.9 to 0.9)
    pub stages: u8,         // Number of all-pass stages (2, 4, 6, 8, 12)
    pub mix: f32,           // Wet/dry mix
}

impl Default for PhaserParams {
    fn default() -> Self {
        Self {
            rate_hz: 0.5,
            depth: 0.5,
            feedback: 0.5,
            stages: 4,
            mix: 0.5,
        }
    }
}

/// Parameters for flanger effect
#[derive(Clone, Debug)]
pub struct FlangerParams {
    pub rate_hz: f32,       // Sweep rate
    pub depth_ms: f32,      // Delay sweep depth in ms
    pub feedback: f32,      // Feedback (-0.9 to 0.9)
    pub mix: f32,           // Wet/dry mix
}

impl Default for FlangerParams {
    fn default() -> Self {
        Self {
            rate_hz: 0.3,
            depth_ms: 2.0,
            feedback: 0.5,
            mix: 0.5,
        }
    }
}

/// Parameters for distortion effect
#[derive(Clone, Debug)]
pub struct DistortionParams {
    pub drive: f32,         // Input gain/drive (1.0 to 100.0)
    pub tone: f32,          // Tone control (0.0 = dark, 1.0 = bright)
    pub output_gain: f32,   // Output level compensation
    pub mix: f32,           // Wet/dry mix
    pub distortion_type: DistortionType,
}

impl Default for DistortionParams {
    fn default() -> Self {
        Self {
            drive: 1.0,
            tone: 0.5,
            output_gain: 1.0,
            mix: 1.0,
            distortion_type: DistortionType::SoftClip,
        }
    }
}

/// Types of distortion
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistortionType {
    SoftClip,       // Smooth saturation
    HardClip,       // Digital clipping
    Foldback,       // Wave folding
    Tube,           // Asymmetric tube-like
    Fuzz,           // Square-ish fuzz
}

impl Default for DistortionType {
    fn default() -> Self {
        Self::SoftClip
    }
}

/// Parameters for bitcrush effect
#[derive(Clone, Debug)]
pub struct BitcrushParams {
    pub bits: u8,           // Bit depth (1-16)
    pub sample_rate_reduction: f32, // Downsampling factor (1.0 = none)
    pub mix: f32,           // Wet/dry mix
    pub dither: bool,       // Add noise dither
}

impl Default for BitcrushParams {
    fn default() -> Self {
        Self {
            bits: 16,
            sample_rate_reduction: 1.0,
            mix: 1.0,
            dither: false,
        }
    }
}

/// Parameters for compressor effect
#[derive(Clone, Debug)]
pub struct CompressorParams {
    pub threshold_db: f32,  // Threshold in dB
    pub ratio: f32,         // Compression ratio (1:1 to inf:1)
    pub attack_ms: f32,     // Attack time in ms
    pub release_ms: f32,    // Release time in ms
    pub knee_db: f32,       // Soft knee width in dB
    pub makeup_gain_db: f32, // Output gain compensation
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            threshold_db: -10.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            knee_db: 3.0,
            makeup_gain_db: 0.0,
        }
    }
}

/// Parameters for reverb effect
#[derive(Clone, Debug)]
pub struct ReverbParams {
    pub room_size: f32,     // Room size (0.0 to 1.0)
    pub decay: f32,         // Decay time in seconds
    pub damping: f32,       // High-frequency damping (0.0 to 1.0)
    pub predelay_ms: f32,   // Pre-delay in milliseconds
    pub mix: f32,           // Wet/dry mix
    pub early_mix: f32,     // Early reflections mix
    pub width: f32,         // Stereo width (0.0 to 1.0)
    pub reverb_type: ReverbType,
}

impl Default for ReverbParams {
    fn default() -> Self {
        Self {
            room_size: 0.5,
            decay: 1.5,
            damping: 0.5,
            predelay_ms: 20.0,
            mix: 0.3,
            early_mix: 0.5,
            width: 1.0,
            reverb_type: ReverbType::Hall,
        }
    }
}

/// Types of reverb algorithms
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReverbType {
    Room,       // Small room
    Hall,       // Concert hall
    Plate,      // Plate reverb
    Spring,     // Spring reverb
    Cave,       // Large cave/cathedral
}

impl Default for ReverbType {
    fn default() -> Self {
        Self::Hall
    }
}

/// Parameters for delay effect
#[derive(Clone, Debug)]
pub struct DelayParams {
    pub time_ms: f32,       // Delay time in milliseconds
    pub time_sync: Option<f32>, // Sync to tempo (beats)
    pub feedback: f32,      // Feedback amount (0.0 to 0.95)
    pub mix: f32,           // Wet/dry mix
    pub ping_pong: bool,    // Ping-pong stereo mode
    pub filter_cutoff: f32, // Feedback filter cutoff (Hz, 0 = off)
    pub filter_type: FilterType,
}

impl Default for DelayParams {
    fn default() -> Self {
        Self {
            time_ms: 250.0,
            time_sync: None,
            feedback: 0.3,
            mix: 0.5,
            ping_pong: false,
            filter_cutoff: 0.0,
            filter_type: FilterType::LowPass,
        }
    }
}

/// Parameters for filter effect
#[derive(Clone, Debug)]
pub struct FilterParams {
    pub cutoff_hz: f32,     // Cutoff frequency
    pub resonance: f32,     // Q factor / resonance (0.1 to 20)
    pub filter_type: FilterType,
    pub drive: f32,         // Pre-filter saturation
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            cutoff_hz: 1000.0,
            resonance: 0.707,
            filter_type: FilterType::LowPass,
            drive: 1.0,
        }
    }
}

/// Types of filters
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,       // Parametric EQ peak
    LowShelf,
    HighShelf,
}

impl Default for FilterType {
    fn default() -> Self {
        Self::LowPass
    }
}

/// LFO waveforms for modulation effects
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Square,
    Saw,
    Random,     // Sample & hold noise
}

impl Default for LfoWaveform {
    fn default() -> Self {
        Self::Sine
    }
}

/// Parameters for pitch shift effect
#[derive(Clone, Debug)]
pub struct PitchShiftParams {
    pub semitones: f32,     // Pitch shift in semitones
    pub cents: f32,         // Fine pitch shift in cents
    pub mix: f32,           // Wet/dry mix
    pub window_ms: f32,     // Processing window size
    pub formant_preserve: bool, // Try to preserve formants
}

impl Default for PitchShiftParams {
    fn default() -> Self {
        Self {
            semitones: 0.0,
            cents: 0.0,
            mix: 1.0,
            window_ms: 50.0,
            formant_preserve: false,
        }
    }
}

// ============================================================================
// EFFECT BUFFERS
// ============================================================================
//
// Circular buffers and state for delay-based effects
// ============================================================================

/// A circular buffer for delay-based effects
#[derive(Clone, Debug)]
pub struct EffectBuffer {
    pub data: Vec<f32>,
    pub write_pos: usize,
    pub size: usize,
}

impl EffectBuffer {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0.0; size.max(1)],
            write_pos: 0,
            size: size.max(1),
        }
    }

    /// Write a sample and advance the write position
    pub fn write(&mut self, sample: f32) {
        self.data[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.size;
    }

    /// Read from a delay (in samples) behind the write position
    pub fn read(&self, delay_samples: usize) -> f32 {
        let delay = delay_samples.min(self.size - 1);
        let read_pos = (self.write_pos + self.size - delay) % self.size;
        self.data[read_pos]
    }

    /// Read with fractional delay using linear interpolation
    pub fn read_interpolated(&self, delay_samples: f32) -> f32 {
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples.fract();

        let sample1 = self.read(delay_int);
        let sample2 = self.read(delay_int + 1);

        sample1 + (sample2 - sample1) * delay_frac
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.data.fill(0.0);
        self.write_pos = 0;
    }

    /// Resize the buffer (clears contents)
    pub fn resize(&mut self, new_size: usize) {
        self.size = new_size.max(1);
        self.data = vec![0.0; self.size];
        self.write_pos = 0;
    }
}

/// Stereo buffer pair for delay effects
#[derive(Clone, Debug)]
pub struct StereoBuffer {
    pub left: EffectBuffer,
    pub right: EffectBuffer,
}

impl StereoBuffer {
    pub fn new(size: usize) -> Self {
        Self {
            left: EffectBuffer::new(size),
            right: EffectBuffer::new(size),
        }
    }

    pub fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
    }
}

// ============================================================================
// UNIFIED EFFECT STATE
// ============================================================================
//
// Combined state for all effect parameters - used by channels and master
// ============================================================================

/// Combined parameters for all effect types
#[derive(Clone, Debug, Default)]
pub struct EffectParameters {
    pub amplitude: AmplitudeParams,
    pub pan: PanParams,
    pub vibrato: VibratoParams,
    pub tremolo: TremoloParams,
    pub chorus: ChorusParams,
    pub phaser: PhaserParams,
    pub flanger: FlangerParams,
    pub distortion: DistortionParams,
    pub bitcrush: BitcrushParams,
    pub compressor: CompressorParams,
    pub reverb: ReverbParams,
    pub delay: DelayParams,
    pub filter: FilterParams,
    pub pitch_shift: PitchShiftParams,
}

/// Combined runtime state for effects (buffers, phases, etc.)
#[derive(Clone, Debug)]
pub struct EffectState {
    // LFO phases
    pub vibrato_phase: f32,
    pub tremolo_phase: f32,
    pub chorus_phase: f32,
    pub phaser_phase: f32,
    pub flanger_phase: f32,

    // Filter states (biquad coefficients and history)
    pub filter_z1: f32,
    pub filter_z2: f32,
    pub filter_z1_r: f32,  // Right channel
    pub filter_z2_r: f32,

    // Compressor envelope follower
    pub compressor_envelope: f32,

    // Bitcrush sample hold
    pub bitcrush_hold_left: f32,
    pub bitcrush_hold_right: f32,
    pub bitcrush_counter: f32,

    // Delay buffers
    pub delay_buffer: StereoBuffer,

    // Chorus buffers
    pub chorus_buffer: StereoBuffer,

    // Reverb state
    pub reverb_early_buffers: Vec<EffectBuffer>,
    pub reverb_comb_buffers: Vec<EffectBuffer>,
    pub reverb_comb_filters: Vec<f32>,
    pub reverb_allpass_buffers: Vec<EffectBuffer>,

    // Phaser all-pass states
    pub phaser_allpass_states: Vec<f32>,

    // Flanger buffer
    pub flanger_buffer: StereoBuffer,

    // Pitch shifter state
    pub pitch_shift_buffer: EffectBuffer,
    pub pitch_shift_read_pos: f32,

    // Random state for S&H LFO
    pub random_state: u32,
    pub random_hold_value: f32,
}

impl EffectState {
    /// Create new effect state with specified buffer sizes
    pub fn new(sample_rate: u32, max_delay_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_delay_seconds) as usize;
        let chorus_samples = (sample_rate as f32 * 0.1) as usize; // 100ms max

        // Reverb buffer sizes (prime-number-like delays)
        let early_times = [7, 11, 13, 17, 19, 23];
        let comb_times = [30, 37, 41, 44, 48, 53, 59, 67];

        Self {
            vibrato_phase: 0.0,
            tremolo_phase: 0.0,
            chorus_phase: 0.0,
            phaser_phase: 0.0,
            flanger_phase: 0.0,

            filter_z1: 0.0,
            filter_z2: 0.0,
            filter_z1_r: 0.0,
            filter_z2_r: 0.0,

            compressor_envelope: 0.0,

            bitcrush_hold_left: 0.0,
            bitcrush_hold_right: 0.0,
            bitcrush_counter: 0.0,

            delay_buffer: StereoBuffer::new(max_samples),
            chorus_buffer: StereoBuffer::new(chorus_samples),

            reverb_early_buffers: early_times
                .iter()
                .map(|&ms| EffectBuffer::new((ms as f32 / 1000.0 * sample_rate as f32 * 2.0) as usize))
                .collect(),
            reverb_comb_buffers: comb_times
                .iter()
                .map(|&ms| EffectBuffer::new((ms as f32 / 1000.0 * sample_rate as f32 * 2.0) as usize))
                .collect(),
            reverb_comb_filters: vec![0.0; comb_times.len()],
            reverb_allpass_buffers: vec![
                EffectBuffer::new((5.0 / 1000.0 * sample_rate as f32) as usize),
                EffectBuffer::new((1.7 / 1000.0 * sample_rate as f32) as usize),
            ],

            phaser_allpass_states: vec![0.0; 12], // Max 12 stages

            flanger_buffer: StereoBuffer::new((20.0 / 1000.0 * sample_rate as f32) as usize),

            pitch_shift_buffer: EffectBuffer::new((100.0 / 1000.0 * sample_rate as f32) as usize),
            pitch_shift_read_pos: 0.0,

            random_state: 12345,
            random_hold_value: 0.0,
        }
    }

    /// Reset all state (clear buffers, reset phases)
    pub fn reset(&mut self) {
        self.vibrato_phase = 0.0;
        self.tremolo_phase = 0.0;
        self.chorus_phase = 0.0;
        self.phaser_phase = 0.0;
        self.flanger_phase = 0.0;

        self.filter_z1 = 0.0;
        self.filter_z2 = 0.0;
        self.filter_z1_r = 0.0;
        self.filter_z2_r = 0.0;

        self.compressor_envelope = 0.0;

        self.bitcrush_hold_left = 0.0;
        self.bitcrush_hold_right = 0.0;
        self.bitcrush_counter = 0.0;

        self.delay_buffer.clear();
        self.chorus_buffer.clear();
        self.flanger_buffer.clear();

        for buf in &mut self.reverb_early_buffers {
            buf.clear();
        }
        for buf in &mut self.reverb_comb_buffers {
            buf.clear();
        }
        self.reverb_comb_filters.fill(0.0);
        for buf in &mut self.reverb_allpass_buffers {
            buf.clear();
        }

        self.phaser_allpass_states.fill(0.0);

        self.pitch_shift_buffer.clear();
        self.pitch_shift_read_pos = 0.0;
    }

    /// Generate a random number (simple LCG)
    pub fn random(&mut self) -> f32 {
        self.random_state = self.random_state.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.random_state >> 16) & 0x7FFF) as f32 / 32767.0
    }
}

impl Default for EffectState {
    fn default() -> Self {
        Self::new(48000, 4.0)
    }
}

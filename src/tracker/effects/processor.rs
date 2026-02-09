// ============================================================================
// PROCESSOR.RS - Effect Chain Processing
// ============================================================================
//
// This module provides the infrastructure for chaining effects together
// and processing audio through effect chains in the correct order.
//
// Key concepts:
// - EffectType: Enum of all available effect types
// - EffectChain: Ordered list of effects to apply
// - process_effect_chain: Apply a chain to audio samples
//
// ============================================================================

use super::{Effect, StereoSample, EffectContext};
use super::core::*;
use super::modulation::*;
use super::dynamics::*;
use super::spatial::*;
use super::filters::*;

// ============================================================================
// EFFECT TYPE ENUMERATION
// ============================================================================
//
// All available effect types. Used for serialization, effect ordering,
// and dynamic effect creation.
// ============================================================================

/// Enumeration of all available effect types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectType {
    // Core
    Amplitude,
    Pan,
    StereoWidth,
    DcFilter,

    // Modulation
    Vibrato,
    Tremolo,
    Chorus,
    Phaser,
    Flanger,
    AutoPan,

    // Dynamics
    Distortion,
    Bitcrush,
    Compressor,
    Limiter,
    Gate,

    // Spatial
    Reverb,
    Delay,
    Echo,
    MultiTapDelay,

    // Filters
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
}

impl EffectType {
    /// Get the effect name as used in CSV files
    pub fn name(&self) -> &'static str {
        match self {
            Self::Amplitude => "amplitude",
            Self::Pan => "pan",
            Self::StereoWidth => "width",
            Self::DcFilter => "dc",
            Self::Vibrato => "vibrato",
            Self::Tremolo => "tremolo",
            Self::Chorus => "chorus",
            Self::Phaser => "phaser",
            Self::Flanger => "flanger",
            Self::AutoPan => "autopan",
            Self::Distortion => "distortion",
            Self::Bitcrush => "bitcrush",
            Self::Compressor => "compressor",
            Self::Limiter => "limiter",
            Self::Gate => "gate",
            Self::Reverb => "reverb",
            Self::Delay => "delay",
            Self::Echo => "echo",
            Self::MultiTapDelay => "multitap",
            Self::LowPass => "lowpass",
            Self::HighPass => "highpass",
            Self::BandPass => "bandpass",
            Self::Notch => "notch",
            Self::Peak => "peak",
            Self::LowShelf => "lowshelf",
            Self::HighShelf => "highshelf",
        }
    }

    /// Parse effect type from string name
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();
        match lower.as_str() {
            "amplitude" | "amp" | "a" | "gain" | "volume" | "vol" => Some(Self::Amplitude),
            "pan" | "p" => Some(Self::Pan),
            "width" | "stereo" | "spread" => Some(Self::StereoWidth),
            "dc" | "dcfilter" => Some(Self::DcFilter),
            "vibrato" | "vib" | "v" => Some(Self::Vibrato),
            "tremolo" | "trem" | "t" => Some(Self::Tremolo),
            "chorus" | "ch" => Some(Self::Chorus),
            "phaser" | "phase" | "ph" => Some(Self::Phaser),
            "flanger" | "flange" | "fl" => Some(Self::Flanger),
            "autopan" | "auto" => Some(Self::AutoPan),
            "distortion" | "dist" | "d" => Some(Self::Distortion),
            "bitcrush" | "crush" | "bit" | "bc" => Some(Self::Bitcrush),
            "compressor" | "comp" | "c" => Some(Self::Compressor),
            "limiter" | "limit" | "lim" => Some(Self::Limiter),
            "gate" | "g" => Some(Self::Gate),
            "reverb" | "rv" | "r" | "reverb1" | "reverb2" => Some(Self::Reverb),
            "delay" | "del" => Some(Self::Delay),
            "echo" | "e" => Some(Self::Echo),
            "multitap" | "multi" | "mt" => Some(Self::MultiTapDelay),
            "lowpass" | "lp" | "lpf" => Some(Self::LowPass),
            "highpass" | "hp" | "hpf" => Some(Self::HighPass),
            "bandpass" | "bp" | "bpf" => Some(Self::BandPass),
            "notch" | "n" => Some(Self::Notch),
            "peak" | "eq" => Some(Self::Peak),
            "lowshelf" | "ls" => Some(Self::LowShelf),
            "highshelf" | "hs" => Some(Self::HighShelf),
            _ => None,
        }
    }

    /// Check if this effect works better at the instrument level (before mixing)
    pub fn is_instrument_level(&self) -> bool {
        matches!(self, Self::Vibrato) // Vibrato works best when modulating the oscillator directly
    }

    /// Check if this effect is typically used on the master bus
    pub fn is_master_effect(&self) -> bool {
        matches!(self,
            Self::Reverb | Self::Delay | Self::Echo | Self::MultiTapDelay |
            Self::Compressor | Self::Limiter | Self::StereoWidth
        )
    }

    /// Get all effect types
    pub fn all() -> &'static [Self] {
        &[
            Self::Amplitude, Self::Pan, Self::StereoWidth, Self::DcFilter,
            Self::Vibrato, Self::Tremolo, Self::Chorus, Self::Phaser, Self::Flanger, Self::AutoPan,
            Self::Distortion, Self::Bitcrush, Self::Compressor, Self::Limiter, Self::Gate,
            Self::Reverb, Self::Delay, Self::Echo, Self::MultiTapDelay,
            Self::LowPass, Self::HighPass, Self::BandPass, Self::Notch, Self::Peak, Self::LowShelf, Self::HighShelf,
        ]
    }
}

// ============================================================================
// EFFECT INSTANCE
// ============================================================================
//
// Boxed effect instance that can hold any effect type.
// ============================================================================

/// A boxed effect that can be stored in chains
pub type BoxedEffect = Box<dyn Effect>;

/// Create a new effect instance of the given type
pub fn create_effect(effect_type: EffectType, sample_rate: u32, max_buffer_seconds: f32) -> BoxedEffect {
    match effect_type {
        EffectType::Amplitude => Box::new(AmplitudeEffect::new()),
        EffectType::Pan => Box::new(PanEffect::new()),
        EffectType::StereoWidth => Box::new(StereoWidthEffect::new()),
        EffectType::DcFilter => Box::new(DcFilterEffect::new()),
        EffectType::Vibrato => Box::new(VibratoEffect::new(sample_rate)),
        EffectType::Tremolo => Box::new(TremoloEffect::new()),
        EffectType::Chorus => Box::new(ChorusEffect::new(sample_rate)),
        EffectType::Phaser => Box::new(PhaserEffect::new()),
        EffectType::Flanger => Box::new(FlangerEffect::new(sample_rate)),
        EffectType::AutoPan => Box::new(AutoPanEffect::new()),
        EffectType::Distortion => Box::new(DistortionEffect::new()),
        EffectType::Bitcrush => Box::new(BitcrushEffect::new()),
        EffectType::Compressor => Box::new(CompressorEffect::new()),
        EffectType::Limiter => Box::new(LimiterEffect::new()),
        EffectType::Gate => Box::new(GateEffect::new()),
        EffectType::Reverb => Box::new(ReverbEffect::new(sample_rate, max_buffer_seconds)),
        EffectType::Delay => Box::new(DelayEffect::new(sample_rate, max_buffer_seconds)),
        EffectType::Echo => Box::new(EchoEffect::new(sample_rate, max_buffer_seconds)),
        EffectType::MultiTapDelay => Box::new(MultiTapDelayEffect::new(sample_rate, max_buffer_seconds)),
        EffectType::LowPass => Box::new(FilterEffect::new(sample_rate)),
        EffectType::HighPass => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::HighPass;
            Box::new(f)
        },
        EffectType::BandPass => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::BandPass;
            Box::new(f)
        },
        EffectType::Notch => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::Notch;
            Box::new(f)
        },
        EffectType::Peak => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::Peak;
            Box::new(f)
        },
        EffectType::LowShelf => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::LowShelf;
            Box::new(f)
        },
        EffectType::HighShelf => {
            let mut f = FilterEffect::new(sample_rate);
            f.params.filter_type = super::state::FilterType::HighShelf;
            Box::new(f)
        },
    }
}

// ============================================================================
// EFFECT CHAIN
// ============================================================================
//
// An ordered collection of effects to apply to audio.
// ============================================================================

/// An ordered chain of effects
pub struct EffectChain {
    effects: Vec<BoxedEffect>,
    context: EffectContext,
}

impl EffectChain {
    /// Create a new empty effect chain
    pub fn new(sample_rate: u32, max_buffer_seconds: f32) -> Self {
        Self {
            effects: Vec::new(),
            context: EffectContext::new(sample_rate, max_buffer_seconds),
        }
    }

    /// Add an effect to the end of the chain
    pub fn push(&mut self, effect: BoxedEffect) {
        self.effects.push(effect);
    }

    /// Add an effect by type
    pub fn add(&mut self, effect_type: EffectType) {
        let effect = create_effect(
            effect_type,
            self.context.sample_rate,
            self.context.max_buffer_samples as f32 / self.context.sample_rate as f32,
        );
        self.effects.push(effect);
    }

    /// Remove all effects
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Get the number of effects in the chain
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Process a sample through the effect chain
    pub fn process(&mut self, input: StereoSample) -> StereoSample {
        let mut sample = input;
        for effect in &mut self.effects {
            if effect.is_active() {
                sample = effect.process(sample, &self.context);
            }
        }
        self.context.advance();
        sample
    }

    /// Process with custom context (e.g., with input frequency)
    pub fn process_with_context(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        let mut sample = input;
        for effect in &mut self.effects {
            if effect.is_active() {
                sample = effect.process(sample, ctx);
            }
        }
        sample
    }

    /// Reset all effects in the chain
    pub fn reset(&mut self) {
        for effect in &mut self.effects {
            effect.reset();
        }
        self.context.current_sample = 0;
    }

    /// Set input frequency for effects that use it (like vibrato)
    pub fn set_input_frequency(&mut self, freq: f32) {
        self.context.input_frequency = freq;
    }

    /// Get mutable access to effects for parameter modification
    pub fn effects_mut(&mut self) -> &mut [BoxedEffect] {
        &mut self.effects
    }

    /// Get immutable access to effects
    pub fn effects(&self) -> &[BoxedEffect] {
        &self.effects
    }
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Process a sample through a slice of effects
pub fn process_effect_chain(
    input: StereoSample,
    effects: &mut [BoxedEffect],
    ctx: &EffectContext,
) -> StereoSample {
    let mut sample = input;
    for effect in effects {
        if effect.is_active() {
            sample = effect.process(sample, ctx);
        }
    }
    sample
}

/// Process mono input to stereo through effects
pub fn process_mono_to_stereo(
    input: f32,
    effects: &mut [BoxedEffect],
    ctx: &EffectContext,
) -> StereoSample {
    process_effect_chain(StereoSample::mono(input), effects, ctx)
}

// ============================================================================
// DEFAULT EFFECT CHAINS
// ============================================================================

/// Create a standard channel effect chain
pub fn create_channel_chain(sample_rate: u32, max_buffer_seconds: f32) -> EffectChain {
    let mut chain = EffectChain::new(sample_rate, max_buffer_seconds);
    // Default channel effects: amplitude and pan always available
    chain.add(EffectType::Amplitude);
    chain.add(EffectType::Pan);
    chain
}

/// Create a standard master bus effect chain
pub fn create_master_chain(sample_rate: u32, max_buffer_seconds: f32) -> EffectChain {
    let mut chain = EffectChain::new(sample_rate, max_buffer_seconds);
    // Default master effects
    chain.add(EffectType::Amplitude);
    chain.add(EffectType::Pan);
    // Reverb and delay often go at the end
    chain
}

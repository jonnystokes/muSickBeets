// ============================================================================
// SPATIAL.RS - Space and Time Effects (Reverb, Delay)
// ============================================================================
//
// Effects that create a sense of space or add rhythmic echoes:
// - Reverb: Simulates room acoustics (multiple algorithms)
// - Delay: Echo/repeat effects
// - Echo: Simple single-tap delay
//
// ============================================================================

use super::{Effect, StereoSample, EffectContext, lerp};
use super::state::{ReverbParams, ReverbType, DelayParams, FilterType, EffectBuffer};

// ============================================================================
// REVERB EFFECT
// ============================================================================
//
// High-quality reverb using Schroeder/Moorer style algorithm with:
// - Early reflections for initial room impression
// - Parallel comb filters for reverb density
// - Series all-pass filters for diffusion
// - Damping for high-frequency absorption
// ============================================================================

/// Reverb effect with multiple algorithms
pub struct ReverbEffect {
    pub params: ReverbParams,
    pub active: bool,

    // Early reflection delay lines
    early_buffers: Vec<EffectBuffer>,
    early_delays_base: Vec<f32>,  // Base delay times in ms
    early_gains: Vec<f32>,

    // Comb filter delay lines (creates density)
    comb_buffers: Vec<EffectBuffer>,
    comb_delays_base: Vec<f32>,  // Base delay times in ms
    comb_filter_states: Vec<f32>,  // For damping

    // All-pass filters (diffusion)
    allpass_buffers: Vec<EffectBuffer>,
    allpass_delays: Vec<f32>,

    // Pre-delay buffer
    predelay_buffer: EffectBuffer,

    sample_rate: u32,
}

impl ReverbEffect {
    pub fn new(sample_rate: u32, max_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_seconds) as usize;

        // Early reflection times (prime-ish numbers for less resonance)
        let early_delays = vec![7.0, 11.0, 13.0, 17.0, 19.0, 23.0, 29.0, 31.0];
        let early_gains = vec![0.8, 0.7, 0.65, 0.6, 0.55, 0.5, 0.45, 0.4];

        // Comb filter delays (Schroeder style, mutually prime-ish)
        let comb_delays = vec![29.7, 37.1, 41.1, 43.7, 47.6, 53.0, 59.3, 67.0];
        let num_comb_filters = comb_delays.len();

        // All-pass delays
        let allpass_delays = vec![5.0, 1.7, 0.6];

        Self {
            params: ReverbParams::default(),
            active: true,

            early_buffers: early_delays.iter()
                .map(|&ms| EffectBuffer::new((ms * 4.0 / 1000.0 * sample_rate as f32) as usize))
                .collect(),
            early_delays_base: early_delays,
            early_gains,

            comb_buffers: comb_delays.iter()
                .map(|&ms| EffectBuffer::new((ms * 4.0 / 1000.0 * sample_rate as f32) as usize))
                .collect(),
            comb_delays_base: comb_delays,
            comb_filter_states: vec![0.0; num_comb_filters],

            allpass_buffers: allpass_delays.iter()
                .map(|&ms| EffectBuffer::new((ms * 10.0 / 1000.0 * sample_rate as f32) as usize))
                .collect(),
            allpass_delays,

            predelay_buffer: EffectBuffer::new(max_samples / 2),

            sample_rate,
        }
    }

    /// Process comb filter with damping
    fn process_comb(&mut self, input: f32, index: usize, feedback: f32) -> f32 {
        let delay_samples = (self.comb_delays_base[index] * self.params.room_size * 2.0
            / 1000.0 * self.sample_rate as f32) as usize;
        let delay_samples = delay_samples.clamp(1, self.comb_buffers[index].size - 1);

        // Read delayed sample
        let delayed = self.comb_buffers[index].read(delay_samples);

        // Apply damping (low-pass filter)
        self.comb_filter_states[index] = lerp(
            delayed,
            self.comb_filter_states[index],
            self.params.damping
        );

        // Write input + filtered feedback to buffer
        self.comb_buffers[index].write(input + self.comb_filter_states[index] * feedback);

        delayed
    }

    /// Process all-pass filter
    fn process_allpass(&mut self, input: f32, index: usize) -> f32 {
        let delay_samples = (self.allpass_delays[index] * self.params.room_size * 5.0
            / 1000.0 * self.sample_rate as f32) as usize;
        let delay_samples = delay_samples.clamp(1, self.allpass_buffers[index].size - 1);

        let delayed = self.allpass_buffers[index].read(delay_samples);
        let g = 0.5; // All-pass coefficient

        let output = -input * g + delayed;
        self.allpass_buffers[index].write(input + delayed * g);

        output
    }
}

impl Effect for ReverbEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.mix == 0.0 {
            return input;
        }

        let mono = input.to_mono();

        // Pre-delay
        let predelay_samples = (self.params.predelay_ms / 1000.0 * ctx.sample_rate as f32) as usize;
        let predelay_samples = predelay_samples.clamp(1, self.predelay_buffer.size - 1);
        self.predelay_buffer.write(mono);
        let predelayed = self.predelay_buffer.read(predelay_samples);

        // Early reflections
        let mut early = 0.0;
        for (i, (buffer, &gain)) in self.early_buffers.iter_mut()
            .zip(self.early_gains.iter()).enumerate()
        {
            let delay_samples = (self.early_delays_base[i] * self.params.room_size * 2.0
                / 1000.0 * ctx.sample_rate as f32) as usize;
            let delay_samples = delay_samples.clamp(1, buffer.size - 1);

            buffer.write(predelayed);
            early += buffer.read(delay_samples) * gain;
        }
        early /= self.early_buffers.len() as f32;

        // Calculate feedback based on decay time
        // RT60 formula: feedback = 10^(-3 * delay_time / RT60)
        let avg_delay_sec = 0.05 * self.params.room_size;
        let feedback = if self.params.decay > 0.0 {
            10.0_f32.powf(-3.0 * avg_delay_sec / self.params.decay).min(0.98)
        } else {
            0.5
        };

        // Comb filters in parallel
        let mut comb_sum = 0.0;
        let comb_input = predelayed + early * self.params.early_mix * 0.3;
        for i in 0..self.comb_buffers.len() {
            comb_sum += self.process_comb(comb_input, i, feedback);
        }
        comb_sum /= self.comb_buffers.len() as f32;

        // All-pass filters in series
        let mut diffused = comb_sum;
        for i in 0..self.allpass_buffers.len() {
            diffused = self.process_allpass(diffused, i);
        }

        // Combine early and late reflections
        let reverb = early * self.params.early_mix + diffused * (1.0 - self.params.early_mix * 0.5);

        // Stereo width (decorrelate L/R slightly)
        let width = self.params.width;
        let reverb_l = reverb;
        let reverb_r = if width > 0.0 && !self.allpass_buffers.is_empty() {
            // Use a different all-pass delay for right channel variation
            self.process_allpass(reverb, 0) * width + reverb * (1.0 - width)
        } else {
            reverb
        };

        // Mix dry and wet
        StereoSample {
            left: input.left * (1.0 - self.params.mix) + reverb_l * self.params.mix,
            right: input.right * (1.0 - self.params.mix) + reverb_r * self.params.mix,
        }
    }

    fn reset(&mut self) {
        for buf in &mut self.early_buffers {
            buf.clear();
        }
        for buf in &mut self.comb_buffers {
            buf.clear();
        }
        self.comb_filter_states.fill(0.0);
        for buf in &mut self.allpass_buffers {
            buf.clear();
        }
        self.predelay_buffer.clear();
    }

    fn name(&self) -> &'static str {
        "reverb"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// DELAY EFFECT
// ============================================================================
//
// Flexible delay with ping-pong mode, feedback filtering, and tempo sync.
// ============================================================================

/// Delay effect
pub struct DelayEffect {
    pub params: DelayParams,
    pub active: bool,
    buffer_left: EffectBuffer,
    buffer_right: EffectBuffer,
    // Simple one-pole filter states for feedback filtering
    filter_state_left: f32,
    filter_state_right: f32,
    sample_rate: u32,
}

impl DelayEffect {
    pub fn new(sample_rate: u32, max_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_seconds) as usize;
        Self {
            params: DelayParams::default(),
            active: true,
            buffer_left: EffectBuffer::new(max_samples),
            buffer_right: EffectBuffer::new(max_samples),
            filter_state_left: 0.0,
            filter_state_right: 0.0,
            sample_rate,
        }
    }

}

/// Apply a simple one-pole low-pass filter (free function to avoid borrow issues)
#[inline]
fn delay_filter(sample: f32, state: &mut f32, filter_cutoff: f32, sample_rate: u32) -> f32 {
    if filter_cutoff <= 0.0 {
        return sample;
    }

    // Simple one-pole low-pass filter
    let coef = (-2.0 * std::f32::consts::PI * filter_cutoff / sample_rate as f32).exp();

    *state = sample + coef * (*state - sample);
    *state
}

impl Effect for DelayEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.params.mix == 0.0 {
            return input;
        }

        // Calculate delay in samples
        let delay_samples = (self.params.time_ms / 1000.0 * ctx.sample_rate as f32) as usize;
        let delay_samples = delay_samples.clamp(1, self.buffer_left.size - 1);

        // Read delayed samples
        let delayed_l = self.buffer_left.read(delay_samples);
        let delayed_r = self.buffer_right.read(delay_samples);

        // Apply feedback filter
        let filtered_l = delay_filter(delayed_l, &mut self.filter_state_left, self.params.filter_cutoff, self.sample_rate);
        let filtered_r = delay_filter(delayed_r, &mut self.filter_state_right, self.params.filter_cutoff, self.sample_rate);

        // Write to buffers with feedback
        if self.params.ping_pong {
            // Ping-pong: cross-feed between channels
            self.buffer_left.write(input.left + filtered_r * self.params.feedback);
            self.buffer_right.write(input.right + filtered_l * self.params.feedback);
        } else {
            // Normal stereo delay
            self.buffer_left.write(input.left + filtered_l * self.params.feedback);
            self.buffer_right.write(input.right + filtered_r * self.params.feedback);
        }

        // Mix
        StereoSample {
            left: input.left * (1.0 - self.params.mix) + delayed_l * self.params.mix,
            right: input.right * (1.0 - self.params.mix) + delayed_r * self.params.mix,
        }
    }

    fn reset(&mut self) {
        self.buffer_left.clear();
        self.buffer_right.clear();
        self.filter_state_left = 0.0;
        self.filter_state_right = 0.0;
    }

    fn name(&self) -> &'static str {
        "delay"
    }

    fn is_active(&self) -> bool {
        self.active && self.params.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.params.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.params.mix
    }
}

// ============================================================================
// SIMPLE ECHO EFFECT
// ============================================================================
//
// Simplified single-tap echo for quick echo effects.
// ============================================================================

/// Simple echo effect
pub struct EchoEffect {
    pub delay_ms: f32,
    pub feedback: f32,
    pub mix: f32,
    pub active: bool,
    buffer: EffectBuffer,
}

impl EchoEffect {
    pub fn new(sample_rate: u32, max_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_seconds) as usize;
        Self {
            delay_ms: 250.0,
            feedback: 0.3,
            mix: 0.5,
            active: true,
            buffer: EffectBuffer::new(max_samples),
        }
    }
}

impl Effect for EchoEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.mix == 0.0 {
            return input;
        }

        let mono = input.to_mono();
        let delay_samples = (self.delay_ms / 1000.0 * ctx.sample_rate as f32) as usize;
        let delay_samples = delay_samples.clamp(1, self.buffer.size - 1);

        let delayed = self.buffer.read(delay_samples);
        self.buffer.write(mono + delayed * self.feedback);

        let wet = StereoSample::mono(delayed);

        StereoSample {
            left: input.left * (1.0 - self.mix) + wet.left * self.mix,
            right: input.right * (1.0 - self.mix) + wet.right * self.mix,
        }
    }

    fn reset(&mut self) {
        self.buffer.clear();
    }

    fn name(&self) -> &'static str {
        "echo"
    }

    fn is_active(&self) -> bool {
        self.active && self.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

// ============================================================================
// MULTI-TAP DELAY
// ============================================================================
//
// Delay with multiple taps at different times and levels.
// ============================================================================

/// Multi-tap delay effect
pub struct MultiTapDelayEffect {
    pub taps: Vec<(f32, f32)>,  // (delay_ms, gain)
    pub feedback: f32,
    pub mix: f32,
    pub active: bool,
    buffer: EffectBuffer,
}

impl MultiTapDelayEffect {
    pub fn new(sample_rate: u32, max_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_seconds) as usize;
        Self {
            taps: vec![(125.0, 0.7), (250.0, 0.5), (375.0, 0.35), (500.0, 0.2)],
            feedback: 0.2,
            mix: 0.5,
            active: true,
            buffer: EffectBuffer::new(max_samples),
        }
    }
}

impl Effect for MultiTapDelayEffect {
    fn process(&mut self, input: StereoSample, ctx: &EffectContext) -> StereoSample {
        if !self.active || self.mix == 0.0 || self.taps.is_empty() {
            return input;
        }

        let mono = input.to_mono();

        // Sum all taps
        let mut tap_sum = 0.0;
        for &(delay_ms, gain) in &self.taps {
            let delay_samples = (delay_ms / 1000.0 * ctx.sample_rate as f32) as usize;
            let delay_samples = delay_samples.clamp(1, self.buffer.size - 1);
            tap_sum += self.buffer.read(delay_samples) * gain;
        }

        // Write input + feedback to buffer
        self.buffer.write(mono + tap_sum * self.feedback);

        let wet = StereoSample::mono(tap_sum);

        StereoSample {
            left: input.left * (1.0 - self.mix) + wet.left * self.mix,
            right: input.right * (1.0 - self.mix) + wet.right * self.mix,
        }
    }

    fn reset(&mut self) {
        self.buffer.clear();
    }

    fn name(&self) -> &'static str {
        "multitap"
    }

    fn is_active(&self) -> bool {
        self.active && self.mix > 0.0
    }

    fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    fn get_mix(&self) -> f32 {
        self.mix
    }
}

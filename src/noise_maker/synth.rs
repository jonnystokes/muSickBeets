use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Result;
use miniaudio::{Device, DeviceConfig, DeviceType, Format};
use realfft::{num_complex::Complex32, RealFftPlanner};

use crate::app_state::{EngineKind, WindowKind};
use crate::frame_file::FrameFile;

pub struct LoopMetrics {
    pub samples: Vec<f32>,
    pub hop_samples: usize,
    pub boundary_jump: f32,
    pub fade_samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone)]
struct Oscillator {
    frequency_hz: f32,
    amplitude: f32,
    phase_rad: f32,
    initial_phase_rad: f32,
}

#[derive(Clone)]
enum PlaybackSource {
    Empty,
    OscBank(Vec<Oscillator>),
    LoopBuffer {
        samples: Arc<Vec<f32>>,
        position: usize,
    },
}

struct SynthInner {
    source: PlaybackSource,
    sample_rate: u32,
    state: PlaybackState,
    user_gain: f32,
    base_gain: f32,
}

fn lock_inner(mutex: &Mutex<SynthInner>) -> MutexGuard<'_, SynthInner> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct SynthPlayer {
    device: Option<Device>,
    device_sample_rate: u32,
    inner: Arc<Mutex<SynthInner>>,
}

impl SynthPlayer {
    pub fn new() -> Self {
        Self {
            device: None,
            device_sample_rate: 0,
            inner: Arc::new(Mutex::new(SynthInner {
                source: PlaybackSource::Empty,
                sample_rate: 44100,
                state: PlaybackState::Stopped,
                user_gain: 1.0,
                base_gain: 1.0,
            })),
        }
    }

    pub fn load_frame_with_engine(
        &mut self,
        frame: &FrameFile,
        engine: EngineKind,
        window: WindowKind,
        overlap_percent: f32,
    ) -> Result<()> {
        {
            let mut inner = lock_inner(&self.inner);
            inner.sample_rate = frame.sample_rate;
            inner.state = PlaybackState::Stopped;
            match engine {
                EngineKind::OscBank => {
                    let oscillators: Vec<Oscillator> = frame
                        .bins
                        .iter()
                        .map(|bin| Oscillator {
                            frequency_hz: bin.frequency_hz,
                            amplitude: bin.magnitude,
                            phase_rad: bin.phase_rad,
                            initial_phase_rad: bin.phase_rad,
                        })
                        .collect();
                    let sum_amp: f32 = oscillators.iter().map(|o| o.amplitude.abs()).sum();
                    inner.base_gain = if sum_amp > 0.8 { 0.8 / sum_amp } else { 1.0 };
                    let peak_amp = oscillators
                        .iter()
                        .map(|o| o.amplitude.abs())
                        .fold(0.0_f32, f32::max);
                    app_log!(
                        "noise_maker",
                        "Synth load OscBank: oscillators={} peak_amp={:.6} sum_amp={:.6} base_gain={:.6} sr={}",
                        oscillators.len(),
                        peak_amp,
                        sum_amp,
                        inner.base_gain,
                        frame.sample_rate,
                    );
                    inner.source = PlaybackSource::OscBank(oscillators);
                }
                EngineKind::FrameOla => {
                    let rendered = render_engine_buffer(
                        frame,
                        engine,
                        window,
                        overlap_percent,
                        frame.sample_rate as usize * 6,
                    )?;
                    let peak = rendered.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
                    inner.base_gain = if peak > 1e-9 { 0.98 / peak } else { 1.0 };
                    app_log!(
                        "noise_maker",
                        "Synth load FrameOla direct: buf_len={} peak={:.6} base_gain={:.6} sr={}",
                        rendered.len(),
                        peak,
                        inner.base_gain,
                        frame.sample_rate,
                    );
                    inner.source = PlaybackSource::LoopBuffer {
                        samples: Arc::new(rendered),
                        position: 0,
                    };
                }
            }
        }

        let sample_rate = frame.sample_rate;
        let need_new_device = self.device.is_none() || self.device_sample_rate != sample_rate;
        if need_new_device {
            self.device = None;
            self.init_device(sample_rate)?;
            self.device_sample_rate = sample_rate;
        }
        Ok(())
    }

    pub fn load_loop_buffer(&mut self, sample_rate: u32, samples: Vec<f32>) -> Result<()> {
        {
            let mut inner = lock_inner(&self.inner);
            inner.sample_rate = sample_rate;
            inner.state = PlaybackState::Stopped;
            let peak = samples.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
            inner.base_gain = if peak > 1e-9 { 0.98 / peak } else { 1.0 };
            app_log!(
                "noise_maker",
                "Synth load loop buffer: len={} peak={:.6} base_gain={:.6} sr={}",
                samples.len(),
                peak,
                inner.base_gain,
                sample_rate,
            );
            inner.source = PlaybackSource::LoopBuffer {
                samples: Arc::new(samples),
                position: 0,
            };
        }

        let need_new_device = self.device.is_none() || self.device_sample_rate != sample_rate;
        if need_new_device {
            self.device = None;
            self.init_device(sample_rate)?;
            self.device_sample_rate = sample_rate;
        }
        Ok(())
    }

    fn init_device(&mut self, sample_rate: u32) -> Result<()> {
        let inner = Arc::clone(&self.inner);
        let mut config = DeviceConfig::new(DeviceType::Playback);
        config.playback_mut().set_format(Format::F32);
        config.playback_mut().set_channels(1);
        config.set_sample_rate(sample_rate);

        config.set_data_callback(move |_device, output, _input| {
            let mut inner = lock_inner(&inner);
            let frames = output.as_samples_mut::<f32>();
            if inner.state != PlaybackState::Playing {
                for sample in frames {
                    *sample = 0.0;
                }
                return;
            }

            let sr = inner.sample_rate as f32;
            let gain = inner.base_gain * inner.user_gain;
            match &mut inner.source {
                PlaybackSource::Empty => {
                    for sample in frames {
                        *sample = 0.0;
                    }
                }
                PlaybackSource::OscBank(oscillators) => {
                    for sample in frames {
                        let mut out = 0.0_f32;
                        for osc in oscillators.iter_mut() {
                            out += osc.amplitude * osc.phase_rad.sin();
                            osc.phase_rad += std::f32::consts::TAU * osc.frequency_hz / sr;
                            if osc.phase_rad > std::f32::consts::TAU {
                                osc.phase_rad -= std::f32::consts::TAU;
                            }
                        }
                        *sample = (out * gain).clamp(-1.0, 1.0);
                    }
                }
                PlaybackSource::LoopBuffer { samples, position } => {
                    if samples.is_empty() {
                        for sample in frames {
                            *sample = 0.0;
                        }
                    } else {
                        for sample in frames {
                            *sample = (samples[*position] * gain).clamp(-1.0, 1.0);
                            *position += 1;
                            if *position >= samples.len() {
                                *position = 0;
                            }
                        }
                    }
                }
            }
        });

        let device = Device::new(None, &config)
            .map_err(|e| anyhow::anyhow!("Failed to create audio device: {:?}", e))?;
        device
            .start()
            .map_err(|e| anyhow::anyhow!("Failed to start audio device: {:?}", e))?;
        self.device = Some(device);
        Ok(())
    }

    pub fn set_gain(&mut self, gain: f32) {
        lock_inner(&self.inner).user_gain = gain;
    }

    pub fn play(&mut self) {
        app_log!("noise_maker", "Playback -> Play");
        lock_inner(&self.inner).state = PlaybackState::Playing;
    }
    pub fn pause(&mut self) {
        app_log!("noise_maker", "Playback -> Pause");
        lock_inner(&self.inner).state = PlaybackState::Paused;
    }
    pub fn stop(&mut self) {
        app_log!("noise_maker", "Playback -> Stop");
        let mut inner = lock_inner(&self.inner);
        inner.state = PlaybackState::Stopped;
        match &mut inner.source {
            PlaybackSource::OscBank(oscillators) => {
                for osc in oscillators.iter_mut() {
                    osc.phase_rad = osc.initial_phase_rad;
                }
            }
            PlaybackSource::LoopBuffer { position, .. } => *position = 0,
            PlaybackSource::Empty => {}
        }
    }
    pub fn get_state(&self) -> PlaybackState {
        lock_inner(&self.inner).state
    }
    pub fn has_frame(&self) -> bool {
        !matches!(lock_inner(&self.inner).source, PlaybackSource::Empty)
    }

    #[allow(dead_code)]
    pub fn preview_samples(&self, count: usize) -> Vec<f32> {
        let inner = lock_inner(&self.inner);
        match &inner.source {
            PlaybackSource::Empty => vec![0.0; count],
            PlaybackSource::OscBank(oscillators) => {
                let sr = inner.sample_rate as f32;
                let gain = inner.base_gain * inner.user_gain;
                let mut phases: Vec<f32> =
                    oscillators.iter().map(|o| o.initial_phase_rad).collect();
                let mut out = vec![0.0_f32; count];
                for sample in &mut out {
                    let mut v = 0.0_f32;
                    for (osc, phase) in oscillators.iter().zip(phases.iter_mut()) {
                        v += osc.amplitude * phase.sin();
                        *phase += std::f32::consts::TAU * osc.frequency_hz / sr;
                        if *phase > std::f32::consts::TAU {
                            *phase -= std::f32::consts::TAU;
                        }
                    }
                    *sample = (v * gain).clamp(-1.0, 1.0);
                }
                out
            }
            PlaybackSource::LoopBuffer { samples, .. } => {
                samples.iter().copied().cycle().take(count).collect()
            }
        }
    }
}

fn kaiser_i0(x: f32) -> f32 {
    let y = x * x / 4.0;
    let mut sum = 1.0_f32;
    let mut term = 1.0_f32;
    for k in 1..20 {
        term *= y / (k as f32 * k as f32);
        sum += term;
    }
    sum
}

fn window_value(kind: WindowKind, n: usize, len: usize) -> f32 {
    if len <= 1 {
        return 1.0;
    }
    let x = n as f32 / (len - 1) as f32;
    match kind {
        WindowKind::Rectangular => 1.0,
        WindowKind::Hann => 0.5 - 0.5 * (std::f32::consts::TAU * x).cos(),
        WindowKind::Hamming => 0.54 - 0.46 * (std::f32::consts::TAU * x).cos(),
        WindowKind::Blackman => {
            0.42 - 0.5 * (std::f32::consts::TAU * x).cos()
                + 0.08 * (2.0 * std::f32::consts::TAU * x).cos()
        }
        WindowKind::Kaiser => {
            let beta = 8.6_f32;
            let t = 2.0 * x - 1.0;
            kaiser_i0(beta * (1.0 - t * t).sqrt()) / kaiser_i0(beta)
        }
    }
}

#[allow(dead_code)]
fn one_frame_ifft(frame: &FrameFile) -> Result<Vec<f32>> {
    let n = frame.fft_size.max(2);
    let mut planner = RealFftPlanner::<f32>::new();
    let c2r = planner.plan_fft_inverse(n);
    let mut spectrum = c2r.make_input_vec();
    for bin in &frame.bins {
        if bin.bin_index < spectrum.len() {
            let mut re = bin.magnitude * bin.phase_rad.cos();
            let mut im = bin.magnitude * bin.phase_rad.sin();
            if bin.bin_index == 0 || bin.bin_index + 1 == spectrum.len() {
                im = 0.0;
            }
            if !re.is_finite() {
                re = 0.0;
            }
            if !im.is_finite() {
                im = 0.0;
            }
            spectrum[bin.bin_index] = Complex32::new(re, im);
        }
    }
    let mut output = c2r.make_output_vec();
    c2r.process(&mut spectrum, &mut output)
        .map_err(|e| anyhow::anyhow!("IFFT failed: {}", e))?;
    for s in &mut output {
        *s /= n as f32;
    }
    Ok(output)
}

fn one_frame_additive(frame: &FrameFile) -> Vec<f32> {
    let len = frame.fft_size.max(2);
    let sr = frame.sample_rate.max(1) as f32;
    let mut out = vec![0.0_f32; len];

    for sample_idx in 0..len {
        let t = sample_idx as f32 / sr;
        let mut v = 0.0_f32;
        for bin in &frame.bins {
            v += bin.magnitude
                * (bin.phase_rad + std::f32::consts::TAU * bin.frequency_hz * t).cos();
        }
        out[sample_idx] = v;
    }

    out
}

pub fn build_frame_ola_loop(
    frame: &FrameFile,
    window: WindowKind,
    overlap_percent: f32,
) -> Result<LoopMetrics> {
    // The exported frame stores active spectral bins chosen by the analyzer for
    // instrument playback. For the frame-loop engine, using a direct additive
    // synthesis frame better matches those sparse-bin amplitudes than treating
    // the data as a tiny sparse IFFT spectrum.
    let frame_td = one_frame_additive(frame);
    let len = frame_td.len().max(2);
    let hop =
        ((len as f32 * (1.0 - overlap_percent.clamp(0.0, 95.0) / 100.0)).round() as usize).max(1);
    let repeats = 24usize;
    let total_len = len + hop * repeats;
    let mut out = vec![0.0_f32; total_len];
    let mut norm = vec![0.0_f32; total_len];
    let window_vec: Vec<f32> = (0..len).map(|i| window_value(window, i, len)).collect();
    for rep in 0..repeats {
        let start = rep * hop;
        for i in 0..len {
            if start + i < total_len {
                let w = window_vec[i];
                out[start + i] += frame_td[i] * w;
                norm[start + i] += w;
            }
        }
    }
    for (s, nrm) in out.iter_mut().zip(norm.iter()) {
        if *nrm > 1e-6 {
            *s /= *nrm;
        }
    }

    // Search a handful of hop-aligned candidate loop lengths and keep the one
    // with the best wrap-point behavior.
    let stable_start = (hop * 8).min(total_len.saturating_sub(hop.max(1)));
    let mut best_samples = Vec::new();
    let mut best_jump = f32::MAX;
    let mut best_fade = 0usize;

    for hops_in_loop in 1..=8usize {
        let candidate_len = (hop * hops_in_loop)
            .max(256)
            .min(total_len.saturating_sub(stable_start))
            .max(hop.max(1));
        if stable_start + candidate_len > total_len || candidate_len < 2 {
            continue;
        }

        let mut candidate = out[stable_start..stable_start + candidate_len].to_vec();
        let fade_samples = (hop / 2).clamp(16, candidate.len().saturating_div(8).max(16));
        if candidate.len() > fade_samples * 2 {
            for i in 0..fade_samples {
                let a = i as f32 / fade_samples.max(1) as f32;
                let end_idx = candidate.len() - fade_samples + i;
                candidate[end_idx] = candidate[end_idx] * (1.0 - a) + candidate[i] * a;
            }
        }

        let jump = (candidate[0] - candidate[candidate.len() - 1]).abs();
        if jump < best_jump {
            best_jump = jump;
            best_fade = fade_samples;
            best_samples = candidate;
        }
    }

    let samples = if best_samples.is_empty() {
        out[stable_start..total_len].to_vec()
    } else {
        best_samples
    };
    let boundary_jump = if samples.len() >= 2 {
        (samples[0] - samples[samples.len() - 1]).abs()
    } else {
        0.0
    };
    let fade_samples = best_fade;

    let peak = samples.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    app_log!(
        "noise_maker",
        "FrameOla loop build: fft_size={} window={} overlap={:.1}% src_len={} hop={} loop_len={} fade={} peak={:.6} jump={:.6}",
        frame.fft_size,
        window.label(),
        overlap_percent,
        len,
        hop,
        samples.len(),
        fade_samples,
        peak,
        boundary_jump,
    );

    Ok(LoopMetrics {
        samples,
        hop_samples: hop,
        boundary_jump,
        fade_samples,
    })
}

pub fn render_engine_buffer(
    frame: &FrameFile,
    engine: EngineKind,
    window: WindowKind,
    overlap_percent: f32,
    target_len: usize,
) -> Result<Vec<f32>> {
    match engine {
        EngineKind::OscBank => {
            let sr = frame.sample_rate.max(1) as f32;
            let sum_amp: f32 = frame.bins.iter().map(|b| b.magnitude.abs()).sum();
            let base_gain = if sum_amp > 0.8 { 0.8 / sum_amp } else { 1.0 };
            let mut phases: Vec<f32> = frame.bins.iter().map(|b| b.phase_rad).collect();
            let mut out = vec![0.0_f32; target_len];
            for sample in &mut out {
                let mut v = 0.0_f32;
                for (bin, phase) in frame.bins.iter().zip(phases.iter_mut()) {
                    v += bin.magnitude * phase.sin();
                    *phase += std::f32::consts::TAU * bin.frequency_hz / sr;
                    if *phase > std::f32::consts::TAU {
                        *phase -= std::f32::consts::TAU;
                    }
                }
                *sample = (v * base_gain).clamp(-1.0, 1.0);
            }
            Ok(out)
        }
        EngineKind::FrameOla => {
            let loop_metrics = build_frame_ola_loop(frame, window, overlap_percent)?;
            Ok(loop_metrics
                .samples
                .iter()
                .copied()
                .cycle()
                .take(target_len)
                .collect())
        }
    }
}

pub fn preview_samples_for_frame(
    frame: &FrameFile,
    engine: EngineKind,
    window: WindowKind,
    overlap_percent: f32,
    count: usize,
) -> Result<Vec<f32>> {
    render_engine_buffer(frame, engine, window, overlap_percent, count)
}

impl Default for SynthPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_file::{FrameBin, FrameFile};

    fn simple_frame() -> FrameFile {
        FrameFile {
            sample_rate: 48_000,
            fft_size: 1024,
            frame_index: 0,
            frame_time_seconds: 0.0,
            active_bin_count: 2,
            bins: vec![
                FrameBin {
                    bin_index: 10,
                    frequency_hz: 468.75,
                    magnitude: 0.5,
                    phase_rad: 0.0,
                },
                FrameBin {
                    bin_index: 22,
                    frequency_hz: 1031.25,
                    magnitude: 0.25,
                    phase_rad: 0.7,
                },
            ],
        }
    }

    #[test]
    fn osc_bank_render_is_finite_and_nonzero() {
        let frame = simple_frame();
        let out = render_engine_buffer(&frame, EngineKind::OscBank, WindowKind::Hann, 75.0, 4096)
            .expect("osc bank render should succeed");
        assert_eq!(out.len(), 4096);
        assert!(out.iter().all(|v| v.is_finite()));
        let peak = out.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(peak > 0.0001);
        assert!(peak <= 1.0);
    }

    #[test]
    fn frame_ola_render_is_finite_and_nonzero() {
        let frame = simple_frame();
        let out = render_engine_buffer(&frame, EngineKind::FrameOla, WindowKind::Hann, 75.0, 4096)
            .expect("frame ola render should succeed");
        assert_eq!(out.len(), 4096);
        assert!(out.iter().all(|v| v.is_finite()));
        let peak = out.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(peak > 0.000001);
    }

    #[test]
    fn preview_generation_respects_requested_length() {
        let frame = simple_frame();
        let out =
            preview_samples_for_frame(&frame, EngineKind::OscBank, WindowKind::Hann, 75.0, 12345)
                .expect("preview should succeed");
        assert_eq!(out.len(), 12345);
    }

    #[test]
    fn frame_ola_loop_metrics_are_reasonable() {
        let frame = simple_frame();
        let metrics = build_frame_ola_loop(&frame, WindowKind::Hann, 75.0)
            .expect("frame ola loop metrics should succeed");
        assert!(!metrics.samples.is_empty());
        assert!(metrics.hop_samples > 0);
        assert!(metrics.fade_samples > 0);
        assert!(metrics.boundary_jump.is_finite());
        assert!(metrics.boundary_jump <= 1.0);
        assert!(
            metrics.boundary_jump < 0.2,
            "loop boundary jump too large: {}",
            metrics.boundary_jump
        );
    }

    #[test]
    fn frame_ola_render_matrix_stays_finite() {
        let frame = simple_frame();
        let windows = [
            WindowKind::Rectangular,
            WindowKind::Hann,
            WindowKind::Hamming,
            WindowKind::Blackman,
            WindowKind::Kaiser,
        ];
        let overlaps = [0.0_f32, 25.0, 50.0, 75.0, 90.0];

        for window in windows {
            for overlap in overlaps {
                let out = render_engine_buffer(&frame, EngineKind::FrameOla, window, overlap, 2048)
                    .expect("frame ola matrix render should succeed");
                assert_eq!(out.len(), 2048, "wrong length for {:?} {}", window, overlap);
                assert!(
                    out.iter().all(|v| v.is_finite()),
                    "non-finite samples for {:?} {}",
                    window,
                    overlap
                );
                let peak = out.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
                assert!(
                    peak <= 1.0,
                    "clipped samples for {:?} {} peak {}",
                    window,
                    overlap,
                    peak
                );
            }
        }
    }
}

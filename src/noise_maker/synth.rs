use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::Result;
use miniaudio::{Device, DeviceConfig, DeviceType, Format};

use crate::frame_file::FrameFile;

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

struct SynthInner {
    oscillators: Vec<Oscillator>,
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
                oscillators: Vec::new(),
                sample_rate: 44100,
                state: PlaybackState::Stopped,
                user_gain: 1.0,
                base_gain: 0.8,
            })),
        }
    }

    pub fn load_frame(&mut self, frame: &FrameFile) -> Result<()> {
        {
            let mut inner = lock_inner(&self.inner);
            inner.sample_rate = frame.sample_rate;
            inner.oscillators = frame
                .bins
                .iter()
                .map(|bin| Oscillator {
                    frequency_hz: bin.frequency_hz,
                    amplitude: bin.magnitude,
                    phase_rad: bin.phase_rad,
                    initial_phase_rad: bin.phase_rad,
                })
                .collect();
            inner.state = PlaybackState::Stopped;

            let sum_amp: f32 = inner.oscillators.iter().map(|o| o.amplitude.abs()).sum();
            inner.base_gain = if sum_amp > 0.8 { 0.8 / sum_amp } else { 1.0 };
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

            for sample in frames {
                let mut out = 0.0_f32;
                for osc in &mut inner.oscillators {
                    out += osc.amplitude * osc.phase_rad.sin();
                    osc.phase_rad += std::f32::consts::TAU * osc.frequency_hz / sr;
                    if osc.phase_rad > std::f32::consts::TAU {
                        osc.phase_rad -= std::f32::consts::TAU;
                    }
                }
                *sample = (out * gain).clamp(-1.0, 1.0);
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
        lock_inner(&self.inner).state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        lock_inner(&self.inner).state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let mut inner = lock_inner(&self.inner);
        inner.state = PlaybackState::Stopped;
        for osc in &mut inner.oscillators {
            osc.phase_rad = osc.initial_phase_rad;
        }
    }

    pub fn has_frame(&self) -> bool {
        !lock_inner(&self.inner).oscillators.is_empty()
    }

    pub fn preview_samples(&self, count: usize) -> Vec<f32> {
        let inner = lock_inner(&self.inner);
        let sr = inner.sample_rate as f32;
        let gain = inner.base_gain * inner.user_gain;
        let mut phases: Vec<f32> = inner
            .oscillators
            .iter()
            .map(|o| o.initial_phase_rad)
            .collect();
        let mut out = vec![0.0_f32; count];

        for sample in &mut out {
            let mut v = 0.0_f32;
            for (osc, phase) in inner.oscillators.iter().zip(phases.iter_mut()) {
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
}

impl Default for SynthPlayer {
    fn default() -> Self {
        Self::new()
    }
}

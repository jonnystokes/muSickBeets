use miniaudio::{DeviceConfig, DeviceType, Format, Device};
use std::sync::{Arc, Mutex};

use crate::data::AudioData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioPlayer {
    device: Option<Device>,
    playback_data: Arc<Mutex<PlaybackData>>,
}

struct PlaybackData {
    samples: Vec<f32>,
    sample_rate: u32,
    position: usize,
    state: PlaybackState,
    repeat: bool,
    end_sample: usize,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            device: None,
            playback_data: Arc::new(Mutex::new(PlaybackData {
                samples: Vec::new(),
                sample_rate: 48000,
                position: 0,
                state: PlaybackState::Stopped,
                repeat: false,
                end_sample: 0,
            })),
        }
    }

    pub fn load_audio(&mut self, audio: &AudioData) -> anyhow::Result<()> {
        self.stop();

        {
            let mut data = self.playback_data.lock().unwrap();
            data.samples = audio.samples.clone();
            data.sample_rate = audio.sample_rate;
            data.position = 0;
            data.end_sample = audio.samples.len();
        }

        if self.device.is_none() {
            self.init_device(audio.sample_rate)?;
        }

        Ok(())
    }

    fn init_device(&mut self, sample_rate: u32) -> anyhow::Result<()> {
        let playback_data = Arc::clone(&self.playback_data);

        let mut config = DeviceConfig::new(DeviceType::Playback);
        config.playback_mut().set_format(Format::F32);
        config.playback_mut().set_channels(1);
        config.set_sample_rate(sample_rate);

        config.set_data_callback(move |_device, output, _input| {
            let mut data = playback_data.lock().unwrap();

            let frames = output.as_samples_mut::<f32>();

            if data.state != PlaybackState::Playing {
                for sample in frames {
                    *sample = 0.0;
                }
                return;
            }

            for sample in frames {
                if data.position >= data.end_sample {
                    if data.repeat {
                        data.position = 0;
                    } else {
                        data.position = 0;
                        data.state = PlaybackState::Paused;
                        *sample = 0.0;
                        continue;
                    }
                }

                if data.position < data.samples.len() {
                    *sample = data.samples[data.position];
                    data.position += 1;
                } else {
                    *sample = 0.0;
                }
            }
        });

        let device = Device::new(None, &config)
            .map_err(|e| anyhow::anyhow!("Failed to create audio device: {:?}", e))?;

        device.start()
            .map_err(|e| anyhow::anyhow!("Failed to start audio device: {:?}", e))?;

        self.device = Some(device);

        Ok(())
    }

    pub fn play(&mut self) {
        let mut data = self.playback_data.lock().unwrap();
        data.state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        let mut data = self.playback_data.lock().unwrap();
        data.state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let mut data = self.playback_data.lock().unwrap();
        data.state = PlaybackState::Stopped;
        data.position = 0;
    }

    pub fn seek_to(&self, seconds: f64) {
        let mut data = self.playback_data.lock().unwrap();
        let sample = (seconds * data.sample_rate as f64) as usize;
        data.position = sample.min(data.end_sample);
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        let mut data = self.playback_data.lock().unwrap();
        data.repeat = repeat;
    }

    pub fn get_state(&self) -> PlaybackState {
        let data = self.playback_data.lock().unwrap();
        data.state
    }

    pub fn get_position_seconds(&self) -> f64 {
        let data = self.playback_data.lock().unwrap();
        data.position as f64 / data.sample_rate as f64
    }

    pub fn has_audio(&self) -> bool {
        let data = self.playback_data.lock().unwrap();
        !data.samples.is_empty()
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

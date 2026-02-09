
use miniaudio::{DeviceConfig, DeviceType, Format, Device};
use std::sync::{Arc, Mutex};

use super::AudioData;

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
    start_sample: usize,
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
                start_sample: 0,
                end_sample: 0,
            })),
        }
    }

    pub fn load_audio(&mut self, audio: &AudioData, start_sample: usize, end_sample: usize) -> anyhow::Result<()> {
        // Stop current playback
        self.stop();

        let start = start_sample.min(audio.num_samples());
        let end = end_sample.min(audio.num_samples());
        
        // Update playback data
        {
            let mut data = self.playback_data.lock().unwrap();
            data.samples = audio.samples[start..end].to_vec();
            data.sample_rate = audio.sample_rate;
            data.position = 0;
            data.start_sample = 0;
            data.end_sample = data.samples.len();
        }

        // Initialize audio device if not already done
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
            
            // Get output buffer as f32 slice
            let frames = output.as_samples_mut::<f32>();
            
            if data.state != PlaybackState::Playing {
                // Output silence
                for sample in frames {
                    *sample = 0.0;
                }
                return;
            }

            for sample in frames {
                if data.position >= data.end_sample {
                    if data.repeat {
                        data.position = data.start_sample;
                    } else {
                        // In single mode: go to start and pause (don't stop)
                        data.position = data.start_sample;
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
        // Note: position is already at start after reaching end in single mode
    }

    pub fn pause(&mut self) {
        let mut data = self.playback_data.lock().unwrap();
        data.state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let mut data = self.playback_data.lock().unwrap();
        data.state = PlaybackState::Stopped;
        data.position = data.start_sample;
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        let mut data = self.playback_data.lock().unwrap();
        data.repeat = repeat;
    }

    pub fn get_state(&self) -> PlaybackState {
        let data = self.playback_data.lock().unwrap();
        data.state
    }

    pub fn get_position(&self) -> usize {
        let data = self.playback_data.lock().unwrap();
        data.position
    }

    pub fn get_position_seconds(&self) -> f64 {
        let data = self.playback_data.lock().unwrap();
        data.position as f64 / data.sample_rate as f64
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let player = AudioPlayer::new();
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_state_changes() {
        let mut player = AudioPlayer::new();
        
        player.play();
        assert_eq!(player.get_state(), PlaybackState::Playing);
        
        player.pause();
        assert_eq!(player.get_state(), PlaybackState::Paused);
        
        player.stop();
        assert_eq!(player.get_state(), PlaybackState::Stopped);
    }

    #[test]
    fn test_repeat_setting() {
        let mut player = AudioPlayer::new();
        
        player.set_repeat(true);
        // Can't easily test internal state, but this shouldn't panic
        
        player.set_repeat(false);
    }
}


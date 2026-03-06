use miniaudio::{Device, DeviceConfig, DeviceType, Format};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioPlayer {
    device: Option<Device>,
    device_sample_rate: u32,
    playback_data: Arc<Mutex<PlaybackData>>,
}

/// Lock the mutex, recovering from poison rather than panicking.
/// PlaybackData is simple value types (position, state, samples) — a poisoned
/// mutex just means another thread panicked while holding the lock. The data
/// is still usable; we'd rather continue with stale data than crash the
/// audio callback thread.
fn lock_playback(mutex: &Mutex<PlaybackData>) -> MutexGuard<'_, PlaybackData> {
    mutex.lock().unwrap_or_else(|poisoned| {
        app_log!("AudioPlayer", "Warning: mutex was poisoned, recovering");
        poisoned.into_inner()
    })
}

struct PlaybackData {
    samples: Arc<Vec<f32>>,
    sample_rate: u32,
    position: usize,
    state: PlaybackState,
    repeat: bool,
    end_sample: usize,
    is_seeking: bool,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            device: None,
            device_sample_rate: 0,
            playback_data: Arc::new(Mutex::new(PlaybackData {
                samples: Arc::new(Vec::new()),
                sample_rate: 48000,
                position: 0,
                state: PlaybackState::Stopped,
                repeat: false,
                end_sample: 0,
                is_seeking: false,
            })),
        }
    }

    /// Load audio samples for playback. Accepts an `Arc` to avoid cloning the
    /// full sample buffer — the caller and player share the same allocation.
    pub fn load_audio(&mut self, samples: Arc<Vec<f32>>, sample_rate: u32) -> anyhow::Result<()> {
        self.stop();

        let num_samples = samples.len();
        {
            let mut data = lock_playback(&self.playback_data);
            data.samples = samples;
            data.sample_rate = sample_rate;
            data.position = 0;
            data.end_sample = num_samples;
        }

        // Recreate device if none exists or sample rate changed
        let need_new_device = self.device.is_none() || self.device_sample_rate != sample_rate;

        if need_new_device {
            // Drop the old device before creating a new one
            self.device = None;
            self.init_device(sample_rate)?;
            self.device_sample_rate = sample_rate;
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
            let mut data = lock_playback(&playback_data);

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
                    } else if data.is_seeking {
                        // User is dragging cursor near end - don't auto-pause
                        *sample = 0.0;
                        continue;
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

        device
            .start()
            .map_err(|e| anyhow::anyhow!("Failed to start audio device: {:?}", e))?;

        self.device = Some(device);

        Ok(())
    }

    pub fn play(&mut self) {
        let mut data = lock_playback(&self.playback_data);
        data.state = PlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        let mut data = lock_playback(&self.playback_data);
        data.state = PlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        let mut data = lock_playback(&self.playback_data);
        data.state = PlaybackState::Stopped;
        data.position = 0;
    }

    pub fn seek_to(&self, seconds: f64) {
        let mut data = lock_playback(&self.playback_data);
        let sample = (seconds * data.sample_rate as f64) as usize;
        data.position = sample.min(data.end_sample);
    }

    pub fn seek_to_sample(&self, sample: usize) {
        let mut data = lock_playback(&self.playback_data);
        data.position = sample.min(data.end_sample);
    }

    pub fn set_seeking(&self, seeking: bool) {
        let mut data = lock_playback(&self.playback_data);
        data.is_seeking = seeking;
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        let mut data = lock_playback(&self.playback_data);
        data.repeat = repeat;
    }

    pub fn get_state(&self) -> PlaybackState {
        let data = lock_playback(&self.playback_data);
        data.state
    }

    pub fn get_position_samples(&self) -> usize {
        let data = lock_playback(&self.playback_data);
        data.position
    }

    pub fn get_position_seconds(&self) -> f64 {
        let data = lock_playback(&self.playback_data);
        data.position as f64 / data.sample_rate as f64
    }

    pub fn has_audio(&self) -> bool {
        let data = lock_playback(&self.playback_data);
        !data.samples.is_empty()
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

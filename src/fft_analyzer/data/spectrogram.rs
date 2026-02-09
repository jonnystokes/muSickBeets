#[derive(Debug, Clone)]
pub struct FftFrame {
    pub time_seconds: f64,
    pub frequencies: Vec<f32>,
    pub magnitudes: Vec<f32>,
    pub phases: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct Spectrogram {
    pub frames: Vec<FftFrame>,
    pub min_freq: f32,
    pub max_freq: f32,
    pub min_time: f64,
    pub max_time: f64,
}

impl Spectrogram {
    pub fn from_frames(frames: Vec<FftFrame>) -> Self {
        if frames.is_empty() {
            return Self {
                frames: Vec::new(),
                min_freq: 0.0,
                max_freq: 0.0,
                min_time: 0.0,
                max_time: 0.0,
            };
        }

        let min_time = frames.first().unwrap().time_seconds;
        let max_time = frames.last().unwrap().time_seconds;
        let max_freq = frames[0].frequencies.last().copied().unwrap_or(0.0);

        Self {
            frames,
            min_freq: 0.0,
            max_freq,
            min_time,
            max_time,
        }
    }

    #[inline]
    pub fn num_frames(&self) -> usize {
        self.frames.len()
    }

    #[inline]
    pub fn num_bins(&self) -> usize {
        self.frames.first().map(|f| f.magnitudes.len()).unwrap_or(0)
    }

    pub fn duration(&self) -> f64 {
        self.max_time - self.min_time
    }

    #[inline]
    pub fn magnitude_to_db(magnitude: f32) -> f32 {
        20.0 * magnitude.max(1e-10).log10()
    }

    pub fn get_magnitude_at(&self, time_seconds: f64, bin_index: usize) -> Option<f32> {
        let frame_idx = self.frames
            .iter()
            .position(|f| f.time_seconds >= time_seconds)?;

        self.frames.get(frame_idx)
            .and_then(|f| f.magnitudes.get(bin_index).copied())
    }

    pub fn peak_magnitude(&self) -> f32 {
        self.frames
            .iter()
            .flat_map(|f| f.magnitudes.iter())
            .copied()
            .fold(0.0f32, f32::max)
    }

    pub fn freq_at_bin(&self, bin_index: usize) -> Option<f32> {
        self.frames.first()
            .and_then(|f| f.frequencies.get(bin_index).copied())
    }

    pub fn bin_at_freq(&self, freq_hz: f32) -> Option<usize> {
        let frame = self.frames.first()?;
        frame.frequencies
            .iter()
            .position(|&f| f >= freq_hz)
    }

    /// Find the frame index closest to the given time
    pub fn frame_at_time(&self, time_seconds: f64) -> Option<usize> {
        if self.frames.is_empty() {
            return None;
        }
        let idx = self.frames
            .binary_search_by(|f| f.time_seconds.partial_cmp(&time_seconds).unwrap())
            .unwrap_or_else(|i| i.min(self.frames.len() - 1));
        Some(idx)
    }
}

impl Default for Spectrogram {
    fn default() -> Self {
        Self::from_frames(Vec::new())
    }
}

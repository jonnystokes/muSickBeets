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

/// Pre-computed mask: for each frame, which bins are "active" (will be reconstructed).
/// Used by the renderer to dim inactive pixels (Option 2 rendering).
#[derive(Debug, Clone)]
pub struct ActiveMask {
    /// mask[frame_idx][bin_idx] = true if this bin is active
    pub mask: Vec<Vec<bool>>,
    /// Parameters used to generate this mask (for cache invalidation)
    pub freq_count: usize,
    pub freq_min_hz: f32,
    pub freq_max_hz: f32,
    /// Time range for processing
    pub time_min_sec: f64,
    pub time_max_sec: f64,
}

impl ActiveMask {
    /// Build the active mask from spectrogram + reconstruction params.
    /// Same logic as Reconstructor: keep top-N bins in freq range per frame.
    pub fn compute(
        spec: &Spectrogram,
        freq_count: usize,
        freq_min_hz: f32,
        freq_max_hz: f32,
        time_min_sec: f64,
        time_max_sec: f64,
    ) -> Self {
        use rayon::prelude::*;

        let mask: Vec<Vec<bool>> = spec.frames.par_iter().map(|frame| {
            let num_bins = frame.magnitudes.len();
            let mut active = vec![false; num_bins];

            // Check if this frame is within the processing time range
            if frame.time_seconds < time_min_sec || frame.time_seconds > time_max_sec {
                return active;
            }

            // Collect bins within frequency range
            let mut bin_mags: Vec<(usize, f32)> = Vec::new();
            for (i, (&mag, &freq)) in frame.magnitudes.iter()
                .zip(frame.frequencies.iter())
                .enumerate()
            {
                if freq >= freq_min_hz && freq <= freq_max_hz {
                    bin_mags.push((i, mag));
                }
            }

            // Sort by magnitude descending, keep top N
            bin_mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let keep = freq_count.min(bin_mags.len());
            for &(idx, _) in &bin_mags[..keep] {
                active[idx] = true;
            }

            active
        }).collect();

        Self {
            mask,
            freq_count,
            freq_min_hz,
            freq_max_hz,
            time_min_sec,
            time_max_sec,
        }
    }

    /// Check if the mask needs recomputing based on current params.
    pub fn is_valid_for(&self, freq_count: usize, freq_min: f32, freq_max: f32, time_min: f64, time_max: f64) -> bool {
        self.freq_count == freq_count
            && (self.freq_min_hz - freq_min).abs() < 0.01
            && (self.freq_max_hz - freq_max).abs() < 0.01
            && (self.time_min_sec - time_min).abs() < 0.0001
            && (self.time_max_sec - time_max).abs() < 0.0001
    }

    /// Check if a specific bin in a specific frame is active.
    #[inline]
    pub fn is_active(&self, frame_idx: usize, bin_idx: usize) -> bool {
        self.mask.get(frame_idx)
            .and_then(|frame| frame.get(bin_idx))
            .copied()
            .unwrap_or(false)
    }
}

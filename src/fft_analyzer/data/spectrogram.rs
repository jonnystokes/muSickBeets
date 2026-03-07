/// Per-frame FFT data: time position, magnitudes, and phases.
/// Frequency bin values are shared across all frames in a Spectrogram
/// (every frame has the same frequency bins), so they live on the
/// Spectrogram struct instead of being duplicated per frame.
#[derive(Debug, Clone)]
pub struct FftFrame {
    pub time_seconds: f64,
    pub magnitudes: Vec<f32>,
    pub phases: Vec<f32>,
}

/// Collection of FFT frames with shared frequency bin vector.
///
/// All frames in a spectrogram share the same frequency bins
/// (bin_index * freq_resolution), stored once in `frequencies`
/// rather than duplicated per frame (~16 MB savings for 1000
/// frames with 4096 bins).
#[derive(Debug, Clone)]
pub struct Spectrogram {
    pub frames: Vec<FftFrame>,
    /// Frequency value for each bin index, shared across all frames.
    pub frequencies: Vec<f32>,
    pub max_freq: f32,
    pub min_time: f64,
    pub max_time: f64,
}

impl Spectrogram {
    /// Build a Spectrogram from pre-computed frames and a shared frequency vector.
    ///
    /// `frequencies` contains the frequency value for each bin index.
    /// All frames must have magnitudes/phases vectors of the same length as `frequencies`.
    pub fn from_frames_with_frequencies(mut frames: Vec<FftFrame>, frequencies: Vec<f32>) -> Self {
        if frames.is_empty() {
            return Self {
                frames: Vec::new(),
                frequencies,
                max_freq: 0.0,
                min_time: 0.0,
                max_time: 0.0,
            };
        }

        // Defensive sort: guarantee frames are ordered by time.
        // In normal usage frames arrive pre-sorted (rayon preserves index order,
        // CSV import uses BTreeMap), but this is a public API and callers
        // should not be required to uphold a sorting invariant.
        frames.sort_by(|a, b| {
            a.time_seconds
                .partial_cmp(&b.time_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let min_time = frames.first().unwrap().time_seconds;
        let max_time = frames.last().unwrap().time_seconds;
        let max_freq = frequencies.last().copied().unwrap_or(0.0);

        Self {
            frames,
            frequencies,
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
        self.frequencies.len()
    }

    #[inline]
    pub fn magnitude_to_db(magnitude: f32) -> f32 {
        20.0 * magnitude.max(1e-10).log10()
    }

    pub fn bin_at_freq(&self, freq_hz: f32) -> Option<usize> {
        if self.frequencies.is_empty() {
            return None;
        }
        // Binary search: frequencies vec is sorted (bin_index * freq_resolution)
        let idx = self.frequencies.partition_point(|&f| f < freq_hz);
        if idx < self.frequencies.len() {
            Some(idx)
        } else {
            // freq_hz is above all bins — return last bin
            Some(self.frequencies.len() - 1)
        }
    }

    /// Find the maximum magnitude across all frames and bins
    pub fn max_magnitude(&self) -> f32 {
        self.frames
            .iter()
            .flat_map(|f| f.magnitudes.iter())
            .copied()
            .fold(0.0f32, f32::max)
    }

    /// Find the frame index closest to the given time.
    /// Returns None for empty spectrograms or NaN input.
    pub fn frame_at_time(&self, time_seconds: f64) -> Option<usize> {
        if self.frames.is_empty() || time_seconds.is_nan() {
            return None;
        }
        let idx = self
            .frames
            .binary_search_by(|f| {
                f.time_seconds
                    .partial_cmp(&time_seconds)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_else(|i| i.min(self.frames.len() - 1));
        Some(idx)
    }
}

impl Default for Spectrogram {
    fn default() -> Self {
        Self::from_frames_with_frequencies(Vec::new(), Vec::new())
    }
}

// ─── Shared active-bin filtering ──────────────────────────────────────────────

/// Determine which frequency bins are "active" for a single frame,
/// applying both a frequency bandpass filter and a top-N magnitude filter.
///
/// This logic is shared between the spectrogram renderer (which dims inactive
/// bins) and the reconstructor (which zeroes them). Keeping it in one place
/// ensures they always agree on which bins are active.
///
/// Returns a `Vec<bool>` of length `magnitudes.len()`, where `true` = active.
pub fn compute_active_bins(
    magnitudes: &[f32],
    frequencies: &[f32],
    freq_min: f32,
    freq_max: f32,
    freq_count: usize,
) -> Vec<bool> {
    let mut active = vec![false; magnitudes.len()];
    let mut in_range_count = 0usize;

    // Pass 1: mark bins within frequency range
    for (i, &freq) in frequencies.iter().enumerate() {
        if i < magnitudes.len() && freq >= freq_min && freq <= freq_max {
            active[i] = true;
            in_range_count += 1;
        }
    }

    // Pass 2: if freq_count limits to fewer than all in-range bins,
    // keep only the top-N by magnitude
    if freq_count < in_range_count {
        let mut bin_mags: Vec<(usize, f32)> = active
            .iter()
            .enumerate()
            .filter_map(|(i, &is_active)| {
                if is_active {
                    Some((i, magnitudes[i]))
                } else {
                    None
                }
            })
            .collect();
        bin_mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        active.fill(false);
        for &(idx, _) in &bin_mags[..freq_count] {
            active[idx] = true;
        }
    }

    active
}

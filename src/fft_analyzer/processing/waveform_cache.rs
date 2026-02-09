use rayon::prelude::*;

/// Pre-computed min/max peaks for fast waveform rendering.
/// Each entry represents one pixel column's worth of audio.
#[derive(Debug, Clone)]
pub struct WaveformPeaks {
    /// (min, max) pairs for each pixel column
    pub peaks: Vec<(f32, f32)>,
    pub time_start: f64,
    pub time_end: f64,
}

impl WaveformPeaks {
    /// Compute waveform peaks for a given sample slice, producing `width` columns.
    /// Uses rayon for parallel computation across columns.
    pub fn compute(
        samples: &[f32],
        _sample_rate: u32,
        time_start: f64,
        time_end: f64,
        width: usize,
    ) -> Self {
        if samples.is_empty() || width == 0 {
            return Self {
                peaks: vec![(0.0, 0.0); width],
                time_start,
                time_end,
            };
        }

        let _duration = time_end - time_start;
        let total_samples = samples.len();

        let peaks: Vec<(f32, f32)> = (0..width)
            .into_par_iter()
            .map(|col| {
                let t_start = col as f64 / width as f64;
                let t_end = (col + 1) as f64 / width as f64;

                let sample_start = (t_start * total_samples as f64) as usize;
                let sample_end = ((t_end * total_samples as f64) as usize).min(total_samples);

                if sample_start >= sample_end {
                    return (0.0, 0.0);
                }

                let slice = &samples[sample_start..sample_end];
                let mut min = f32::MAX;
                let mut max = f32::MIN;

                for &s in slice {
                    if s < min { min = s; }
                    if s > max { max = s; }
                }

                (min, max)
            })
            .collect();

        Self {
            peaks,
            time_start,
            time_end,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.peaks.is_empty()
    }
}

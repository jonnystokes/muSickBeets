
/// Spectrogram data structure
/// 
/// This module contains only the data representation.
/// Rendering is handled by SpectrogramRenderer for performance.

use super::fft_engine::FftFrame;

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

    /// Convert magnitude to dB scale
    #[inline]
    pub fn magnitude_to_db(magnitude: f32) -> f32 {
        20.0 * magnitude.max(1e-10).log10()
    }

    /// Get the magnitude at a specific time and frequency bin
    pub fn get_magnitude_at(&self, time_seconds: f64, bin_index: usize) -> Option<f32> {
        // Find closest frame
        let frame_idx = self.frames
            .iter()
            .position(|f| f.time_seconds >= time_seconds)?;
        
        self.frames.get(frame_idx)
            .and_then(|f| f.magnitudes.get(bin_index).copied())
    }

    /// Get data for a specific frequency range
    pub fn get_frequency_slice(&self, min_hz: f32, max_hz: f32) -> Vec<Vec<f32>> {
        let mut result = Vec::with_capacity(self.frames.len());

        for frame in &self.frames {
            let mut slice_mags = Vec::new();
            
            for (freq, mag) in frame.frequencies.iter().zip(&frame.magnitudes) {
                if *freq >= min_hz && *freq <= max_hz {
                    slice_mags.push(*mag);
                }
            }
            
            result.push(slice_mags);
        }

        result
    }

    /// Get peak magnitude across all frames
    pub fn peak_magnitude(&self) -> f32 {
        self.frames
            .iter()
            .flat_map(|f| f.magnitudes.iter())
            .copied()
            .fold(0.0f32, f32::max)
    }

    /// Get average magnitude across all frames
    pub fn average_magnitude(&self) -> f32 {
        let (sum, count) = self.frames
            .iter()
            .flat_map(|f| f.magnitudes.iter())
            .fold((0.0f32, 0usize), |(sum, count), &mag| (sum + mag, count + 1));
        
        if count > 0 {
            sum / count as f32
        } else {
            0.0
        }
    }

    /// Find the frequency bin with highest magnitude at a given time
    pub fn peak_frequency_at(&self, time_seconds: f64) -> Option<(f32, f32)> {
        let frame_idx = self.frames
            .iter()
            .position(|f| f.time_seconds >= time_seconds)?;
        
        let frame = self.frames.get(frame_idx)?;
        
        frame.magnitudes
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(idx, &mag)| (frame.frequencies[idx], mag))
    }
}

impl Default for Spectrogram {
    fn default() -> Self {
        Self::from_frames(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_frame(time: f64, num_bins: usize) -> FftFrame {
        FftFrame {
            time_seconds: time,
            frequencies: (0..num_bins).map(|i| i as f32 * 10.0).collect(),
            magnitudes: (0..num_bins).map(|i| (i as f32 / num_bins as f32) * 0.5).collect(),
            phases: vec![0.0; num_bins],
        }
    }

    #[test]
    fn test_magnitude_to_db() {
        assert!((Spectrogram::magnitude_to_db(1.0) - 0.0).abs() < 0.01);
        assert!((Spectrogram::magnitude_to_db(10.0) - 20.0).abs() < 0.01);
        assert!((Spectrogram::magnitude_to_db(0.1) + 20.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_spectrogram() {
        let spec = Spectrogram::from_frames(Vec::new());
        assert_eq!(spec.num_frames(), 0);
        assert_eq!(spec.num_bins(), 0);
        assert_eq!(spec.peak_magnitude(), 0.0);
    }

    #[test]
    fn test_spectrogram_dimensions() {
        let frames = vec![
            make_test_frame(0.0, 512),
            make_test_frame(0.01, 512),
            make_test_frame(0.02, 512),
        ];
        let spec = Spectrogram::from_frames(frames);
        
        assert_eq!(spec.num_frames(), 3);
        assert_eq!(spec.num_bins(), 512);
    }

    #[test]
    fn test_peak_magnitude() {
        let frames = vec![
            make_test_frame(0.0, 100),
            make_test_frame(0.01, 100),
        ];
        let spec = Spectrogram::from_frames(frames);
        
        let peak = spec.peak_magnitude();
        assert!(peak > 0.0);
        assert!(peak <= 0.5);
    }

    #[test]
    fn test_get_magnitude_at() {
        let frames = vec![
            make_test_frame(0.0, 100),
            make_test_frame(0.01, 100),
        ];
        let spec = Spectrogram::from_frames(frames);
        
        let mag = spec.get_magnitude_at(0.0, 50);
        assert!(mag.is_some());
    }
}

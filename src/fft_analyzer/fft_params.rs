
use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowType {
    Hann,
    Hamming,
    Blackman,
    Kaiser(f32), // beta parameter
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeUnit {
    Seconds,
    Samples,
}

#[derive(Debug, Clone)]
pub struct FftParams {
    pub window_length: usize,  // Number of samples in FFT window
    pub overlap_percent: f32,   // 0.0 to 100.0
    pub window_type: WindowType,
    pub use_center: bool,       // Add padding for symmetry
    pub start_time: f64,        // In seconds or samples depending on unit
    pub stop_time: f64,         // In seconds or samples depending on unit
    pub time_unit: TimeUnit,
    pub sample_rate: u32,
}

impl Default for FftParams {
    fn default() -> Self {
        Self {
            window_length: 2048,
            overlap_percent: 75.0,
            window_type: WindowType::Hann,
            use_center: true,
            start_time: 0.0,
            stop_time: 0.0,
            time_unit: TimeUnit::Seconds,
            sample_rate: 48000,
        }
    }
}

impl FftParams {
    pub fn hop_length(&self) -> usize {
        let overlap_ratio = self.overlap_percent / 100.0;
        ((self.window_length as f32) * (1.0 - overlap_ratio)).max(1.0) as usize
    }

    pub fn start_sample(&self) -> usize {
        match self.time_unit {
            TimeUnit::Seconds => (self.start_time * self.sample_rate as f64) as usize,
            TimeUnit::Samples => self.start_time as usize,
        }
    }

    pub fn stop_sample(&self) -> usize {
        match self.time_unit {
            TimeUnit::Seconds => (self.stop_time * self.sample_rate as f64) as usize,
            TimeUnit::Samples => self.stop_time as usize,
        }
    }

    pub fn generate_window(&self) -> Vec<f32> {
        let n = self.window_length;
        let mut window = vec![0.0; n];

        match self.window_type {
            WindowType::Hann => {
                for i in 0..n {
                    window[i] = 0.5 * (1.0 - ((2.0 * PI * i as f32) / (n - 1) as f32).cos());
                }
            }
            WindowType::Hamming => {
                for i in 0..n {
                    window[i] = 0.54 - 0.46 * ((2.0 * PI * i as f32) / (n - 1) as f32).cos();
                }
            }
            WindowType::Blackman => {
                let a0 = 0.42;
                let a1 = 0.5;
                let a2 = 0.08;
                for i in 0..n {
                    let x = (2.0 * PI * i as f32) / (n - 1) as f32;
                    window[i] = a0 - a1 * x.cos() + a2 * (2.0 * x).cos();
                }
            }
            WindowType::Kaiser(beta) => {
                let ino_beta = bessel_i0(beta);
                for i in 0..n {
                    let x = (2.0 * i as f32) / (n - 1) as f32 - 1.0;
                    let arg = beta * (1.0 - x * x).sqrt();
                    window[i] = bessel_i0(arg) / ino_beta;
                }
            }
        }

        window
    }
}

// Modified Bessel function of the first kind, order 0
// Used for Kaiser window
fn bessel_i0(x: f32) -> f32 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let mut k = 1.0;

    loop {
        term *= (x / 2.0) / k;
        term *= (x / 2.0) / k;
        sum += term;
        k += 1.0;

        if term < 1e-12 * sum {
            break;
        }
        if k > 100.0 {
            break;
        }
    }

    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hop_length() {
        let params = FftParams {
            window_length: 2048,
            overlap_percent: 75.0,
            ..Default::default()
        };
        assert_eq!(params.hop_length(), 512);
    }

    #[test]
    fn test_hop_length_zero_overlap() {
        let params = FftParams {
            window_length: 1024,
            overlap_percent: 0.0,
            ..Default::default()
        };
        assert_eq!(params.hop_length(), 1024);
    }

    #[test]
    fn test_hop_length_high_overlap() {
        let params = FftParams {
            window_length: 2048,
            overlap_percent: 95.0,
            ..Default::default()
        };
        assert_eq!(params.hop_length(), 102); // ~5% of 2048
    }

    #[test]
    fn test_window_generation() {
        let params = FftParams {
            window_length: 1024,
            window_type: WindowType::Hann,
            ..Default::default()
        };
        let window = params.generate_window();
        assert_eq!(window.len(), 1024);
        assert!(window[0] < 0.01); // Should be near zero at edges
        assert!(window[512] > 0.99); // Should be near 1.0 at center
    }

    #[test]
    fn test_hamming_window() {
        let params = FftParams {
            window_length: 512,
            window_type: WindowType::Hamming,
            ..Default::default()
        };
        let window = params.generate_window();
        assert_eq!(window.len(), 512);
        // Hamming has non-zero edges
        assert!(window[0] > 0.07 && window[0] < 0.09);
    }

    #[test]
    fn test_blackman_window() {
        let params = FftParams {
            window_length: 256,
            window_type: WindowType::Blackman,
            ..Default::default()
        };
        let window = params.generate_window();
        assert_eq!(window.len(), 256);
        assert!(window[0] < 0.01); // Near zero at edges
    }

    #[test]
    fn test_kaiser_window() {
        let params = FftParams {
            window_length: 256,
            window_type: WindowType::Kaiser(8.6),
            ..Default::default()
        };
        let window = params.generate_window();
        assert_eq!(window.len(), 256);
        assert!(window[128] > 0.99); // Should be ~1.0 at center
    }

    #[test]
    fn test_time_to_sample_seconds() {
        let params = FftParams {
            start_time: 1.0,
            stop_time: 2.0,
            time_unit: TimeUnit::Seconds,
            sample_rate: 48000,
            ..Default::default()
        };
        assert_eq!(params.start_sample(), 48000);
        assert_eq!(params.stop_sample(), 96000);
    }

    #[test]
    fn test_time_to_sample_samples() {
        let params = FftParams {
            start_time: 1000.0,
            stop_time: 2000.0,
            time_unit: TimeUnit::Samples,
            sample_rate: 48000,
            ..Default::default()
        };
        assert_eq!(params.start_sample(), 1000);
        assert_eq!(params.stop_sample(), 2000);
    }
}


use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowType {
    Hann,
    Hamming,
    Blackman,
    Kaiser(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeUnit {
    Seconds,
    Samples,
}

#[derive(Debug, Clone)]
pub struct FftParams {
    pub window_length: usize,
    pub overlap_percent: f32,
    pub window_type: WindowType,
    pub use_center: bool,
    pub start_time: f64,
    pub stop_time: f64,
    pub time_unit: TimeUnit,
    pub sample_rate: u32,
}

impl Default for FftParams {
    fn default() -> Self {
        Self {
            window_length: 8192,
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

    pub fn num_frequency_bins(&self) -> usize {
        self.window_length / 2 + 1
    }

    pub fn frequency_resolution(&self) -> f32 {
        if self.window_length == 0 { return 0.0; }
        self.sample_rate as f32 / self.window_length as f32
    }

    pub fn num_segments(&self, total_samples: usize) -> usize {
        if total_samples < self.window_length {
            return 0;
        }
        let padded = if self.use_center {
            total_samples + self.window_length
        } else {
            total_samples
        };
        (padded.saturating_sub(self.window_length)) / self.hop_length() + 1
    }

    pub fn bin_duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 { return 0.0; }
        self.hop_length() as f64 / self.sample_rate as f64
    }

    pub fn generate_window(&self) -> Vec<f32> {
        let n = self.window_length;
        if n <= 1 { return vec![1.0; n]; }
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

fn bessel_i0(x: f32) -> f32 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let mut k = 1.0;

    loop {
        term *= (x / 2.0) / k;
        term *= (x / 2.0) / k;
        sum += term;
        k += 1.0;

        if term < 1e-12 * sum || k > 100.0 {
            break;
        }
    }

    sum
}

/// A single color stop in a custom gradient (position 0.0..1.0, color as RGB floats 0.0..1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub position: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl GradientStop {
    pub fn new(position: f32, r: f32, g: f32, b: f32) -> Self {
        Self { position: position.clamp(0.0, 1.0), r, g, b }
    }
}

/// Default gradient: SebLague-style rainbow (Black → Purple → Blue → Green → Yellow → Orange → Red)
pub fn default_custom_gradient() -> Vec<GradientStop> {
    vec![
        GradientStop::new(0.0000, 0.00, 0.00, 0.00),
        GradientStop::new(0.2618, 0.27, 0.11, 0.42),
        GradientStop::new(0.4147, 0.17, 0.47, 0.92),
        GradientStop::new(0.6559, 0.34, 0.92, 0.22),
        GradientStop::new(0.7618, 0.88, 0.88, 0.12),
        GradientStop::new(0.8735, 1.00, 0.56, 0.10),
        GradientStop::new(0.9559, 1.00, 0.00, 0.00),
    ]
}

/// Evaluate a custom gradient at position t (0..1) using linear interpolation between stops.
pub fn eval_gradient(stops: &[GradientStop], t: f32) -> (f32, f32, f32) {
    if stops.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    if stops.len() == 1 || t <= stops[0].position {
        return (stops[0].r, stops[0].g, stops[0].b);
    }
    let last = &stops[stops.len() - 1];
    if t >= last.position {
        return (last.r, last.g, last.b);
    }
    // Find bracketing stops
    let mut idx = 0;
    for i in 1..stops.len() {
        if t < stops[i].position {
            break;
        }
        idx = i;
    }
    let s0 = &stops[idx];
    let s1 = &stops[(idx + 1).min(stops.len() - 1)];
    let seg_len = s1.position - s0.position;
    let seg_t = if seg_len.abs() < 1e-6 { 0.0 } else { ((t - s0.position) / seg_len).clamp(0.0, 1.0) };
    (
        s0.r + (s1.r - s0.r) * seg_t,
        s0.g + (s1.g - s0.g) * seg_t,
        s0.b + (s1.b - s0.b) * seg_t,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum FreqScale {
    Linear,
    Log,
    Power(f32), // 0.0 = linear, 1.0 = log, anything between = blend
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColormapId {
    Classic,
    Viridis,
    Magma,
    Inferno,
    Greyscale,
    InvertedGrey,
    Geek,
    Custom,
}

impl ColormapId {
    pub const ALL: &'static [ColormapId] = &[
        ColormapId::Classic,
        ColormapId::Viridis,
        ColormapId::Magma,
        ColormapId::Inferno,
        ColormapId::Greyscale,
        ColormapId::InvertedGrey,
        ColormapId::Geek,
        ColormapId::Custom,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ColormapId::Classic => "Classic",
            ColormapId::Viridis => "Viridis",
            ColormapId::Magma => "Magma",
            ColormapId::Inferno => "Inferno",
            ColormapId::Greyscale => "Greyscale",
            ColormapId::InvertedGrey => "Inverted Grey",
            ColormapId::Geek => "Geek",
            ColormapId::Custom => "Custom",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or(ColormapId::Classic)
    }
}

#[derive(Debug, Clone)]
pub struct ViewState {
    // Frequency axis display range (viewport)
    pub freq_min_hz: f32,
    pub freq_max_hz: f32,
    pub freq_scale: FreqScale,

    // Time axis display range (viewport)
    pub time_min_sec: f64,
    pub time_max_sec: f64,

    // Display parameters
    pub threshold_db: f32,
    pub db_ceiling: f32,
    pub brightness: f32,
    pub gamma: f32,
    pub colormap: ColormapId,

    // Custom gradient (used when colormap == Custom)
    pub custom_gradient: Vec<GradientStop>,

    // Reconstruction / processing parameters
    pub recon_freq_count: usize,
    pub recon_freq_min_hz: f32,
    pub recon_freq_max_hz: f32,

    // Full data bounds (for reset zoom / unlocked scrolling)
    pub data_freq_max_hz: f32,
    pub data_time_min_sec: f64,
    pub data_time_max_sec: f64,
    pub max_freq_bins: usize,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            freq_min_hz: 100.0,
            freq_max_hz: 2000.0,
            freq_scale: FreqScale::Power(0.5),

            time_min_sec: 0.0,
            time_max_sec: 0.0,

            threshold_db: -87.0,
            db_ceiling: 0.0,
            brightness: 1.0,
            gamma: 2.2,
            colormap: ColormapId::Classic,
            custom_gradient: default_custom_gradient(),

            recon_freq_count: 4097,
            recon_freq_min_hz: 0.0,
            recon_freq_max_hz: 5000.0,

            data_freq_max_hz: 5000.0,
            data_time_min_sec: 0.0,
            data_time_max_sec: 0.0,
            max_freq_bins: 4097,
        }
    }
}

impl ViewState {
    /// Map a normalized t (0..1, bottom to top) to frequency in Hz.
    /// Power(p) interpolates between linear (0.0) and log (1.0).
    pub fn y_to_freq(&self, t: f32) -> f32 {
        let min = self.freq_min_hz.max(1.0);
        let max = self.freq_max_hz.max(min + 1.0);

        match self.freq_scale {
            FreqScale::Linear => {
                min + (max - min) * t
            }
            FreqScale::Log => {
                min * (max / min).powf(t)
            }
            FreqScale::Power(power) => {
                let p = power.clamp(0.0, 1.0);
                if p <= 0.001 {
                    min + (max - min) * t
                } else if p >= 0.999 {
                    min * (max / min).powf(t)
                } else {
                    let linear_freq = min + (max - min) * t;
                    let log_freq = min * (max / min).powf(t);
                    // Geometric interpolation for smooth blending
                    linear_freq.powf(1.0 - p) * log_freq.powf(p)
                }
            }
        }
    }

    /// Map a frequency in Hz to normalized t (0..1, bottom to top)
    pub fn freq_to_y(&self, freq_hz: f32) -> f32 {
        let min = self.freq_min_hz.max(1.0);
        let max = self.freq_max_hz.max(min + 1.0);
        if freq_hz <= min { return 0.0; }
        if freq_hz >= max { return 1.0; }

        match self.freq_scale {
            FreqScale::Linear => {
                ((freq_hz - min) / (max - min)).clamp(0.0, 1.0)
            }
            FreqScale::Log => {
                ((freq_hz / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
            }
            FreqScale::Power(power) => {
                let p = power.clamp(0.0, 1.0);
                if p <= 0.001 {
                    ((freq_hz - min) / (max - min)).clamp(0.0, 1.0)
                } else if p >= 0.999 {
                    ((freq_hz / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
                } else {
                    // Binary search for inverse of blended forward mapping
                    let mut lo = 0.0_f32;
                    let mut hi = 1.0_f32;
                    for _ in 0..32 {
                        let mid = (lo + hi) / 2.0;
                        let linear_f = min + (max - min) * mid;
                        let log_f = min * (max / min).powf(mid);
                        let f = linear_f.powf(1.0 - p) * log_f.powf(p);
                        if f < freq_hz {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    ((lo + hi) / 2.0).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// Map a normalized t (0..1) to time in seconds
    pub fn x_to_time(&self, t: f64) -> f64 {
        self.time_min_sec + (self.time_max_sec - self.time_min_sec) * t
    }

    /// Map time in seconds to normalized t (0..1)
    pub fn time_to_x(&self, time_sec: f64) -> f64 {
        let range = self.time_max_sec - self.time_min_sec;
        if range <= 0.0 { return 0.0; }
        ((time_sec - self.time_min_sec) / range).clamp(0.0, 1.0)
    }

    pub fn reset_zoom(&mut self) {
        self.freq_min_hz = 0.0;
        self.freq_max_hz = self.data_freq_max_hz;
        self.time_min_sec = self.data_time_min_sec;
        self.time_max_sec = self.data_time_max_sec;
    }

    pub fn visible_time_range(&self) -> f64 {
        self.time_max_sec - self.time_min_sec
    }

    pub fn visible_freq_range(&self) -> f32 {
        self.freq_max_hz - self.freq_min_hz
    }
}

#[derive(Debug, Clone)]
pub struct TransportState {
    /// Current playback position in samples (global = recon_start + local).
    pub position_samples: usize,
    /// Duration of reconstructed audio in samples.
    pub duration_samples: usize,
    /// Sample rate for seconds conversion at display time.
    pub sample_rate: u32,
    pub is_playing: bool,
    pub repeat: bool,
}

impl TransportState {
    #[allow(dead_code)]
    pub fn position_seconds(&self) -> f64 {
        self.position_samples as f64 / self.sample_rate.max(1) as f64
    }

    pub fn duration_seconds(&self) -> f64 {
        self.duration_samples as f64 / self.sample_rate.max(1) as f64
    }
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            position_samples: 0,
            duration_samples: 0,
            sample_rate: 48000,
            is_playing: false,
            repeat: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FreqScale {
    Linear,
    Log,
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
            freq_min_hz: 0.0,
            freq_max_hz: 5000.0,
            freq_scale: FreqScale::Log,

            time_min_sec: 0.0,
            time_max_sec: 0.0,

            threshold_db: -124.0,
            db_ceiling: 0.0,
            brightness: 1.0,
            gamma: 2.2,
            colormap: ColormapId::Classic,

            recon_freq_count: 513,
            recon_freq_min_hz: 0.0,
            recon_freq_max_hz: 5000.0,

            data_freq_max_hz: 5000.0,
            data_time_min_sec: 0.0,
            data_time_max_sec: 0.0,
            max_freq_bins: 513,
        }
    }
}

impl ViewState {
    /// Map a normalized t (0..1, bottom to top) to frequency in Hz
    pub fn y_to_freq(&self, t: f32) -> f32 {
        match self.freq_scale {
            FreqScale::Linear => {
                self.freq_min_hz + (self.freq_max_hz - self.freq_min_hz) * t
            }
            FreqScale::Log => {
                let min = self.freq_min_hz.max(1.0);
                let max = self.freq_max_hz.max(min + 1.0);
                min * (max / min).powf(t)
            }
        }
    }

    /// Map a frequency in Hz to normalized t (0..1, bottom to top)
    pub fn freq_to_y(&self, freq_hz: f32) -> f32 {
        match self.freq_scale {
            FreqScale::Linear => {
                let range = self.freq_max_hz - self.freq_min_hz;
                if range <= 0.0 { return 0.0; }
                ((freq_hz - self.freq_min_hz) / range).clamp(0.0, 1.0)
            }
            FreqScale::Log => {
                let min = self.freq_min_hz.max(1.0);
                let max = self.freq_max_hz.max(min + 1.0);
                if freq_hz <= min { return 0.0; }
                ((freq_hz / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
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
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub is_playing: bool,
    pub repeat: bool,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            position_seconds: 0.0,
            duration_seconds: 0.0,
            is_playing: false,
            repeat: false,
        }
    }
}

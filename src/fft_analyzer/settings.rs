use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// All application settings, loaded from INI file or defaults.
/// Every field here is saveable/loadable.
#[derive(Debug, Clone)]
pub struct Settings {
    // ── Analysis ──
    pub window_length: usize,
    pub overlap_percent: f32,
    pub window_type: String, // "Hann", "Hamming", "Blackman", "Kaiser"
    pub kaiser_beta: f32,
    pub center_pad: bool,
    pub zero_pad_factor: usize,
    pub target_segments_per_active: usize,
    pub target_bins_per_segment: usize,
    pub last_edited_field: String, // "Overlap", "SegmentsPerActive", "BinsPerSegment"

    // ── View: Frequency ──
    pub view_freq_min_hz: f32,
    pub view_freq_max_hz: f32,
    pub freq_scale_power: f32, // 0.0 = linear, 1.0 = log, anything in between

    // ── View: Display ──
    pub colormap: String, // "Classic", "Viridis", etc.
    pub threshold_db: f32,
    pub db_ceiling: f32,
    pub brightness: f32,
    pub gamma: f32,

    // ── Reconstruction ──
    pub recon_freq_min_hz: f32,
    pub recon_freq_max_hz: f32,
    pub recon_freq_count: usize,

    // ── Audio ──
    pub normalize_audio: bool,
    pub normalize_peak: f32, // 0.97 = 97% of max

    // ── Zoom ──
    pub time_zoom_factor: f32, // multiplier per click, e.g. 1.5
    pub freq_zoom_factor: f32,
    pub mouse_zoom_factor: f32, // for scroll wheel

    // ── Window ──
    pub window_width: i32,
    pub window_height: i32,
    pub sidebar_width: i32,

    // ── Axis Labels ──
    pub axis_font_size: i32,
    pub freq_axis_width: i32,
    pub time_axis_height: i32,

    // ── Waveform ──
    pub waveform_height: i32,

    // ── Tooltips ──
    pub show_tooltips: bool,
    pub lock_to_active: bool,

    // ── Playback ──
    pub repeat_playback: bool,

    // ── Custom Gradient ──
    /// Serialized as "pos:r:g:b|pos:r:g:b|..." (all floats 0..1)
    pub custom_gradient: String,

    // ── Colors (hex) ──
    pub color_background: u32,
    pub color_panel: u32,
    pub color_widget: u32,
    pub color_text_primary: u32,
    pub color_text_secondary: u32,
    pub color_accent: u32,
    pub color_waveform: u32,
    pub color_cursor: u32,
    pub color_center_line: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Analysis
            window_length: 8192,
            overlap_percent: 75.0,
            window_type: "Hann".to_string(),
            kaiser_beta: 8.6,
            center_pad: false,
            zero_pad_factor: 1,
            target_segments_per_active: 0,
            target_bins_per_segment: 0,
            last_edited_field: "Overlap".to_string(),

            // View: Frequency
            view_freq_min_hz: 100.0,
            view_freq_max_hz: 2000.0,
            freq_scale_power: 0.5, // halfway between linear and log

            // View: Display
            colormap: "Classic".to_string(),
            threshold_db: -87.0,
            db_ceiling: 0.0,
            brightness: 1.0,
            gamma: 2.2,

            // Reconstruction
            recon_freq_min_hz: 0.0,
            recon_freq_max_hz: 5000.0,
            recon_freq_count: 4097,

            // Audio
            normalize_audio: true,
            normalize_peak: 0.97,

            // Zoom
            time_zoom_factor: 1.5,
            freq_zoom_factor: 1.5,
            mouse_zoom_factor: 1.2,

            // Window
            window_width: 1200,
            window_height: 1555,
            sidebar_width: 215,

            // Axis Labels
            axis_font_size: 9,
            freq_axis_width: 50,
            time_axis_height: 20,

            // Waveform
            waveform_height: 100,

            // Tooltips
            show_tooltips: true,
            lock_to_active: false,

            // Playback
            repeat_playback: false,

            // Custom Gradient (default: SebLague classic)
            custom_gradient: String::new(),

            // Colors
            color_background: 0x1e1e2e,
            color_panel: 0x313244,
            color_widget: 0x45475a,
            color_text_primary: 0xcdd6f4,
            color_text_secondary: 0xa6adc8,
            color_accent: 0x89b4fa,
            color_waveform: 0x89b4fa,
            color_cursor: 0xf38ba8,
            color_center_line: 0x45475a,
        }
    }
}

use crate::app_state::AppState;
use crate::data::{default_custom_gradient, FreqScale, GradientStop};

#[allow(dead_code)]
impl Settings {
    const FILE_NAME: &'static str = "settings.ini";

    /// Load settings from INI file, or create it with defaults if it doesn't exist.
    pub fn load_or_create() -> Self {
        // Migrate from old filename if needed
        let old_path = Path::new("muSickBeets.ini");
        let path = Path::new(Self::FILE_NAME);
        if !path.exists() && old_path.exists() {
            eprintln!("[Settings] Migrating muSickBeets.ini -> settings.ini");
            let _ = fs::rename(old_path, path);
        }
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => {
                    let mut settings = Self::default();
                    settings.parse_ini(&content);
                    settings
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Could not read {}: {}. Using defaults.",
                        Self::FILE_NAME,
                        e
                    );
                    let settings = Self::default();
                    settings.save();
                    settings
                }
            }
        } else {
            let settings = Self::default();
            settings.save();
            settings
        }
    }

    /// Create Settings from current AppState (for Save As Default)
    pub fn from_app_state(st: &AppState) -> Self {
        let mut cfg = Self::default();

        // Analysis
        cfg.window_length = st.fft_params.window_length;
        cfg.overlap_percent = st.fft_params.overlap_percent;
        cfg.window_type = match st.fft_params.window_type {
            crate::data::WindowType::Hann => "Hann".to_string(),
            crate::data::WindowType::Hamming => "Hamming".to_string(),
            crate::data::WindowType::Blackman => "Blackman".to_string(),
            crate::data::WindowType::Kaiser(b) => {
                cfg.kaiser_beta = b;
                "Kaiser".to_string()
            }
        };
        cfg.center_pad = st.fft_params.use_center;
        cfg.zero_pad_factor = st.fft_params.zero_pad_factor;
        cfg.target_segments_per_active = st.fft_params.target_segments_per_active.unwrap_or(0);
        cfg.target_bins_per_segment = st.fft_params.target_bins_per_segment.unwrap_or(0);
        cfg.last_edited_field = match st.fft_params.last_edited_field {
            crate::data::LastEditedField::Overlap => "Overlap".to_string(),
            crate::data::LastEditedField::SegmentsPerActive => "SegmentsPerActive".to_string(),
            crate::data::LastEditedField::BinsPerSegment => "BinsPerSegment".to_string(),
        };

        // View
        cfg.view_freq_min_hz = st.view.freq_min_hz;
        cfg.view_freq_max_hz = st.view.freq_max_hz;
        cfg.freq_scale_power = match st.view.freq_scale {
            FreqScale::Linear => 0.0,
            FreqScale::Log => 1.0,
            FreqScale::Power(p) => p,
        };

        // Display
        cfg.colormap = st.view.colormap.name().to_string();
        cfg.threshold_db = st.view.threshold_db;
        cfg.db_ceiling = st.view.db_ceiling;
        cfg.brightness = st.view.brightness;
        cfg.gamma = st.view.gamma;

        // Reconstruction
        cfg.recon_freq_min_hz = st.view.recon_freq_min_hz;
        cfg.recon_freq_max_hz = st.view.recon_freq_max_hz;
        cfg.recon_freq_count = st.view.recon_freq_count;

        // Audio
        cfg.normalize_audio = st.normalize_audio;
        cfg.normalize_peak = st.normalize_peak;

        // Zoom
        cfg.time_zoom_factor = st.time_zoom_factor;
        cfg.freq_zoom_factor = st.freq_zoom_factor;
        cfg.mouse_zoom_factor = st.mouse_zoom_factor;

        // UI
        cfg.lock_to_active = st.lock_to_active;

        // Custom Gradient
        cfg.custom_gradient = serialize_gradient(&st.view.custom_gradient);

        cfg
    }

    /// Save current settings to INI file.
    pub fn save(&self) {
        let content = self.to_ini();
        if let Err(e) = fs::write(Self::FILE_NAME, content) {
            eprintln!("Warning: Could not save {}: {}", Self::FILE_NAME, e);
        }
    }

    fn to_ini(&self) -> String {
        let mut s = String::new();
        s.push_str("# muSickBeets Settings\n");
        s.push_str("# Edit values below. Delete this file to reset to defaults.\n\n");

        s.push_str("[Analysis]\n");
        s.push_str(&format!("window_length = {}\n", self.window_length));
        s.push_str(&format!("overlap_percent = {}\n", self.overlap_percent));
        s.push_str(&format!("window_type = {}\n", self.window_type));
        s.push_str(&format!("kaiser_beta = {}\n", self.kaiser_beta));
        s.push_str(&format!("center_pad = {}\n", self.center_pad));
        s.push_str(&format!("zero_pad_factor = {}\n", self.zero_pad_factor));
        s.push_str(&format!(
            "target_segments_per_active = {}\n",
            self.target_segments_per_active
        ));
        s.push_str(&format!(
            "target_bins_per_segment = {}\n",
            self.target_bins_per_segment
        ));
        s.push_str(&format!("last_edited_field = {}\n", self.last_edited_field));
        s.push('\n');

        s.push_str("[View]\n");
        s.push_str(&format!("view_freq_min_hz = {}\n", self.view_freq_min_hz));
        s.push_str(&format!("view_freq_max_hz = {}\n", self.view_freq_max_hz));
        s.push_str("# freq_scale_power: 0.0 = linear, 1.0 = full log, 0.5 = halfway\n");
        s.push_str(&format!("freq_scale_power = {}\n", self.freq_scale_power));
        s.push('\n');

        s.push_str("[Display]\n");
        s.push_str(
            "# Colormaps: Classic, Viridis, Magma, Inferno, Greyscale, Inverted Grey, Geek\n",
        );
        s.push_str(&format!("colormap = {}\n", self.colormap));
        s.push_str(&format!("threshold_db = {}\n", self.threshold_db));
        s.push_str(&format!("db_ceiling = {}\n", self.db_ceiling));
        s.push_str(&format!("brightness = {}\n", self.brightness));
        s.push_str(&format!("gamma = {}\n", self.gamma));
        s.push('\n');

        s.push_str("[Reconstruction]\n");
        s.push_str(&format!("recon_freq_min_hz = {}\n", self.recon_freq_min_hz));
        s.push_str(&format!("recon_freq_max_hz = {}\n", self.recon_freq_max_hz));
        s.push_str(&format!("recon_freq_count = {}\n", self.recon_freq_count));
        s.push('\n');

        s.push_str("[Audio]\n");
        s.push_str(&format!("normalize_audio = {}\n", self.normalize_audio));
        s.push_str("# normalize_peak: fraction of max (0.97 = 97%)\n");
        s.push_str(&format!("normalize_peak = {}\n", self.normalize_peak));
        s.push('\n');

        s.push_str("[Zoom]\n");
        s.push_str("# zoom factors: how much each click zooms (1.5 = 50% closer/further)\n");
        s.push_str(&format!("time_zoom_factor = {}\n", self.time_zoom_factor));
        s.push_str(&format!("freq_zoom_factor = {}\n", self.freq_zoom_factor));
        s.push_str(&format!("mouse_zoom_factor = {}\n", self.mouse_zoom_factor));
        s.push('\n');

        s.push_str("[Window]\n");
        s.push_str(&format!("window_width = {}\n", self.window_width));
        s.push_str(&format!("window_height = {}\n", self.window_height));
        s.push_str(&format!("sidebar_width = {}\n", self.sidebar_width));
        s.push('\n');

        s.push_str("[AxisLabels]\n");
        s.push_str(&format!("axis_font_size = {}\n", self.axis_font_size));
        s.push_str(&format!("freq_axis_width = {}\n", self.freq_axis_width));
        s.push_str(&format!("time_axis_height = {}\n", self.time_axis_height));
        s.push('\n');

        s.push_str("[Waveform]\n");
        s.push_str(&format!("waveform_height = {}\n", self.waveform_height));
        s.push('\n');

        s.push_str("[UI]\n");
        s.push_str(&format!("show_tooltips = {}\n", self.show_tooltips));
        s.push_str(&format!("lock_to_active = {}\n", self.lock_to_active));
        s.push_str(&format!("repeat_playback = {}\n", self.repeat_playback));
        s.push('\n');

        if !self.custom_gradient.is_empty() {
            s.push_str("[CustomGradient]\n");
            s.push_str("# Format: pos:r:g:b|pos:r:g:b|... (floats 0-1)\n");
            s.push_str(&format!("custom_gradient = {}\n", self.custom_gradient));
            s.push('\n');
        }

        s.push_str("[Colors]\n");
        s.push_str("# Colors are in hex (0xRRGGBB)\n");
        s.push_str(&format!(
            "color_background = 0x{:06x}\n",
            self.color_background
        ));
        s.push_str(&format!("color_panel = 0x{:06x}\n", self.color_panel));
        s.push_str(&format!("color_widget = 0x{:06x}\n", self.color_widget));
        s.push_str(&format!(
            "color_text_primary = 0x{:06x}\n",
            self.color_text_primary
        ));
        s.push_str(&format!(
            "color_text_secondary = 0x{:06x}\n",
            self.color_text_secondary
        ));
        s.push_str(&format!("color_accent = 0x{:06x}\n", self.color_accent));
        s.push_str(&format!("color_waveform = 0x{:06x}\n", self.color_waveform));
        s.push_str(&format!("color_cursor = 0x{:06x}\n", self.color_cursor));
        s.push_str(&format!(
            "color_center_line = 0x{:06x}\n",
            self.color_center_line
        ));

        s
    }

    fn parse_ini(&mut self, content: &str) {
        let map = parse_ini_to_map(content);

        // Analysis
        if let Some(v) = map.get("window_length") {
            if let Ok(n) = v.parse() {
                self.window_length = n;
            }
        }
        if let Some(v) = map.get("overlap_percent") {
            if let Ok(n) = v.parse() {
                self.overlap_percent = n;
            }
        }
        if let Some(v) = map.get("window_type") {
            self.window_type = v.clone();
        }
        if let Some(v) = map.get("kaiser_beta") {
            if let Ok(n) = v.parse() {
                self.kaiser_beta = n;
            }
        }
        if let Some(v) = map.get("center_pad") {
            self.center_pad = v == "true";
        }
        if let Some(v) = map.get("zero_pad_factor") {
            if let Ok(n) = v.parse() {
                self.zero_pad_factor = n;
            }
        }
        if let Some(v) = map.get("target_segments_per_active") {
            if let Ok(n) = v.parse() {
                self.target_segments_per_active = n;
            }
        }
        if let Some(v) = map.get("target_bins_per_segment") {
            if let Ok(n) = v.parse() {
                self.target_bins_per_segment = n;
            }
        }
        if let Some(v) = map.get("last_edited_field") {
            self.last_edited_field = v.clone();
        }

        // View
        if let Some(v) = map.get("view_freq_min_hz") {
            if let Ok(n) = v.parse() {
                self.view_freq_min_hz = n;
            }
        }
        if let Some(v) = map.get("view_freq_max_hz") {
            if let Ok(n) = v.parse() {
                self.view_freq_max_hz = n;
            }
        }
        if let Some(v) = map.get("freq_scale_power") {
            if let Ok(n) = v.parse() {
                self.freq_scale_power = n;
            }
        }

        // Display
        if let Some(v) = map.get("colormap") {
            self.colormap = v.clone();
        }
        if let Some(v) = map.get("threshold_db") {
            if let Ok(n) = v.parse() {
                self.threshold_db = n;
            }
        }
        if let Some(v) = map.get("db_ceiling") {
            if let Ok(n) = v.parse() {
                self.db_ceiling = n;
            }
        }
        if let Some(v) = map.get("brightness") {
            if let Ok(n) = v.parse() {
                self.brightness = n;
            }
        }
        if let Some(v) = map.get("gamma") {
            if let Ok(n) = v.parse() {
                self.gamma = n;
            }
        }

        // Reconstruction
        if let Some(v) = map.get("recon_freq_min_hz") {
            if let Ok(n) = v.parse() {
                self.recon_freq_min_hz = n;
            }
        }
        if let Some(v) = map.get("recon_freq_max_hz") {
            if let Ok(n) = v.parse() {
                self.recon_freq_max_hz = n;
            }
        }
        if let Some(v) = map.get("recon_freq_count") {
            if let Ok(n) = v.parse() {
                self.recon_freq_count = n;
            }
        }

        // Audio
        if let Some(v) = map.get("normalize_audio") {
            self.normalize_audio = v == "true";
        }
        if let Some(v) = map.get("normalize_peak") {
            if let Ok(n) = v.parse() {
                self.normalize_peak = n;
            }
        }

        // Zoom
        if let Some(v) = map.get("time_zoom_factor") {
            if let Ok(n) = v.parse() {
                self.time_zoom_factor = n;
            }
        }
        if let Some(v) = map.get("freq_zoom_factor") {
            if let Ok(n) = v.parse() {
                self.freq_zoom_factor = n;
            }
        }
        if let Some(v) = map.get("mouse_zoom_factor") {
            if let Ok(n) = v.parse() {
                self.mouse_zoom_factor = n;
            }
        }

        // Window
        if let Some(v) = map.get("window_width") {
            if let Ok(n) = v.parse() {
                self.window_width = n;
            }
        }
        if let Some(v) = map.get("window_height") {
            if let Ok(n) = v.parse() {
                self.window_height = n;
            }
        }
        if let Some(v) = map.get("sidebar_width") {
            if let Ok(n) = v.parse() {
                self.sidebar_width = n;
            }
        }

        // Axis Labels
        if let Some(v) = map.get("axis_font_size") {
            if let Ok(n) = v.parse() {
                self.axis_font_size = n;
            }
        }
        if let Some(v) = map.get("freq_axis_width") {
            if let Ok(n) = v.parse() {
                self.freq_axis_width = n;
            }
        }
        if let Some(v) = map.get("time_axis_height") {
            if let Ok(n) = v.parse() {
                self.time_axis_height = n;
            }
        }

        // Waveform
        if let Some(v) = map.get("waveform_height") {
            if let Ok(n) = v.parse() {
                self.waveform_height = n;
            }
        }

        // UI
        if let Some(v) = map.get("show_tooltips") {
            self.show_tooltips = v == "true";
        }
        if let Some(v) = map.get("lock_to_active") {
            self.lock_to_active = v == "true";
        }
        if let Some(v) = map.get("repeat_playback") {
            self.repeat_playback = v == "true";
        }

        // Custom Gradient
        if let Some(v) = map.get("custom_gradient") {
            self.custom_gradient = v.clone();
        }

        // Colors
        if let Some(v) = map.get("color_background") {
            if let Some(n) = parse_hex(v) {
                self.color_background = n;
            }
        }
        if let Some(v) = map.get("color_panel") {
            if let Some(n) = parse_hex(v) {
                self.color_panel = n;
            }
        }
        if let Some(v) = map.get("color_widget") {
            if let Some(n) = parse_hex(v) {
                self.color_widget = n;
            }
        }
        if let Some(v) = map.get("color_text_primary") {
            if let Some(n) = parse_hex(v) {
                self.color_text_primary = n;
            }
        }
        if let Some(v) = map.get("color_text_secondary") {
            if let Some(n) = parse_hex(v) {
                self.color_text_secondary = n;
            }
        }
        if let Some(v) = map.get("color_accent") {
            if let Some(n) = parse_hex(v) {
                self.color_accent = n;
            }
        }
        if let Some(v) = map.get("color_waveform") {
            if let Some(n) = parse_hex(v) {
                self.color_waveform = n;
            }
        }
        if let Some(v) = map.get("color_cursor") {
            if let Some(n) = parse_hex(v) {
                self.color_cursor = n;
            }
        }
        if let Some(v) = map.get("color_center_line") {
            if let Some(n) = parse_hex(v) {
                self.color_center_line = n;
            }
        }
    }

    /// Convert window_type string to the WindowType enum value index
    pub fn window_type_index(&self) -> usize {
        match self.window_type.as_str() {
            "Hann" => 0,
            "Hamming" => 1,
            "Blackman" => 2,
            "Kaiser" => 3,
            _ => 0,
        }
    }

    /// Convert colormap string to ColormapId index
    pub fn colormap_index(&self) -> usize {
        match self.colormap.as_str() {
            "Classic" => 0,
            "Viridis" => 1,
            "Magma" => 2,
            "Inferno" => 3,
            "Greyscale" => 4,
            "Inverted Grey" => 5,
            "Geek" => 6,
            "Custom" => 7,
            _ => 0,
        }
    }

    /// Parse the custom gradient string into GradientStop vec.
    /// Returns default gradient if string is empty or invalid.
    pub fn parse_custom_gradient(&self) -> Vec<GradientStop> {
        deserialize_gradient(&self.custom_gradient)
    }
}

/// Parse INI content into a flat key-value map (section headers are ignored,
/// keys are globally unique in our format).
fn parse_ini_to_map(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim().to_string();
            let val = val.trim().to_string();
            map.insert(key, val);
        }
    }
    map
}

/// Parse a hex string like "0x1e1e2e" or "1e1e2e" into u32.
fn parse_hex(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(s, 16).ok()
}

/// Serialize gradient stops to string: "pos:r:g:b|pos:r:g:b|..."
fn serialize_gradient(stops: &[GradientStop]) -> String {
    stops
        .iter()
        .map(|s| format!("{:.4}:{:.4}:{:.4}:{:.4}", s.position, s.r, s.g, s.b))
        .collect::<Vec<_>>()
        .join("|")
}

/// Deserialize gradient stops from string. Returns default gradient on failure.
fn deserialize_gradient(s: &str) -> Vec<GradientStop> {
    if s.trim().is_empty() {
        return default_custom_gradient();
    }
    let mut stops = Vec::new();
    for part in s.split('|') {
        let vals: Vec<f32> = part
            .split(':')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if vals.len() == 4 {
            stops.push(GradientStop::new(vals[0], vals[1], vals[2], vals[3]));
        }
    }
    if stops.len() < 2 {
        return default_custom_gradient();
    }
    stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
    stops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmentation_solver_fields_roundtrip_in_ini_text() {
        let ini = "[Analysis]
window_length = 8192
overlap_percent = 75
window_type = Hann
kaiser_beta = 8.6
center_pad = false
zero_pad_factor = 1
target_segments_per_active = 19
target_bins_per_segment = 513
last_edited_field = BinsPerSegment
";
        let mut restored = Settings::default();
        restored.parse_ini(ini);

        assert_eq!(restored.target_segments_per_active, 19);
        assert_eq!(restored.target_bins_per_segment, 513);
        assert_eq!(restored.last_edited_field, "BinsPerSegment");
    }

    #[test]
    fn segmentation_solver_fields_default_when_missing_in_ini() {
        let ini = "[Analysis]
window_length = 8192
overlap_percent = 75
window_type = Hann
kaiser_beta = 8.6
center_pad = false
zero_pad_factor = 1
";
        let mut restored = Settings::default();
        restored.target_segments_per_active = 99;
        restored.target_bins_per_segment = 999;
        restored.last_edited_field = "BinsPerSegment".to_string();
        restored.parse_ini(ini);

        assert_eq!(restored.target_segments_per_active, 99);
        assert_eq!(restored.target_bins_per_segment, 999);
        assert_eq!(restored.last_edited_field, "BinsPerSegment");
    }
}

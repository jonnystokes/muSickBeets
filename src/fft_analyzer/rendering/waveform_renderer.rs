use fltk::image::RgbImage;
use fltk::prelude::ImageExt;

use crate::processing::waveform_cache::WaveformPeaks;
use crate::data::ViewState;

/// Colors for the dark theme waveform
const BG_COLOR: (u8, u8, u8) = (0x1e, 0x1e, 0x2e);
const WAVE_COLOR: (u8, u8, u8) = (0x89, 0xb4, 0xfa);  // accent blue
const CENTER_LINE_COLOR: (u8, u8, u8) = (0x45, 0x47, 0x5a);
const CURSOR_COLOR: (u8, u8, u8) = (0xf3, 0x8b, 0xa8);  // red-pink

pub struct WaveformRenderer {
    cached_image: Option<RgbImage>,
    cached_buffer: Vec<u8>,
    cache_valid: bool,
    last_size: (i32, i32),
    last_peaks_len: usize,
    last_view_hash: u64,
}

impl WaveformRenderer {
    pub fn new() -> Self {
        Self {
            cached_image: None,
            cached_buffer: Vec::new(),
            cache_valid: false,
            last_size: (0, 0),
            last_peaks_len: 0,
            last_view_hash: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }

    fn view_hash(view: &ViewState, peaks: &WaveformPeaks) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add((view.time_min_sec * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((view.time_max_sec * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((peaks.time_start * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((peaks.time_end * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add(peaks.peaks.len() as u64);
        h
    }

    pub fn draw(
        &mut self,
        peaks: &WaveformPeaks,
        view: &ViewState,
        cursor_x: Option<i32>,
        x: i32, y: i32, w: i32, h: i32,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }

        if peaks.is_empty() {
            self.draw_no_data(x, y, w, h);
            return;
        }

        let hash = Self::view_hash(view, peaks);
        let needs_rebuild = !self.cache_valid
            || self.last_size != (w, h)
            || self.last_peaks_len != peaks.peaks.len()
            || self.last_view_hash != hash;

        if needs_rebuild {
            self.rebuild_cache(peaks, view, w as usize, h as usize);
            self.last_size = (w, h);
            self.last_peaks_len = peaks.peaks.len();
            self.last_view_hash = hash;
            self.cache_valid = true;
        }

        if let Some(ref mut image) = self.cached_image {
            image.draw(x, y, w, h);
        }

        // Draw playback cursor on top
        if let Some(cx) = cursor_x {
            if cx >= 0 && cx < w {
                use fltk::draw;
                draw::set_draw_color(fltk::enums::Color::from_rgb(
                    CURSOR_COLOR.0, CURSOR_COLOR.1, CURSOR_COLOR.2,
                ));
                draw::draw_line(x + cx, y, x + cx, y + h);
            }
        }
    }

    fn draw_no_data(&self, x: i32, y: i32, w: i32, h: i32) {
        use fltk::draw;
        use fltk::enums::Color;
        draw::set_draw_color(Color::from_hex(0x1e1e2e));
        draw::draw_rectf(x, y, w, h);
        draw::set_draw_color(Color::from_hex(0x6c7086));
        draw::set_font(fltk::enums::Font::Helvetica, 11);
        draw::draw_text("Waveform", x + 10, y + h / 2 + 4);
    }

    /// Rebuild waveform image, mapping peaks to viewport time range.
    /// Peaks have their own time_start..time_end (the processing range).
    /// The viewport may show a different time range, so we map correctly.
    fn rebuild_cache(&mut self, peaks: &WaveformPeaks, view: &ViewState, width: usize, height: usize) {
        let buffer_size = width * height * 3;
        if self.cached_buffer.len() != buffer_size {
            self.cached_buffer = vec![0u8; buffer_size];
        }

        let center_y = height / 2;

        // Fill background
        for i in 0..width * height {
            let idx = i * 3;
            self.cached_buffer[idx] = BG_COLOR.0;
            self.cached_buffer[idx + 1] = BG_COLOR.1;
            self.cached_buffer[idx + 2] = BG_COLOR.2;
        }

        // Draw center line
        for px in 0..width {
            let idx = (center_y * width + px) * 3;
            self.cached_buffer[idx] = CENTER_LINE_COLOR.0;
            self.cached_buffer[idx + 1] = CENTER_LINE_COLOR.1;
            self.cached_buffer[idx + 2] = CENTER_LINE_COLOR.2;
        }

        // Draw waveform peaks, mapping audio time to viewport pixels
        let num_peaks = peaks.peaks.len();
        if num_peaks == 0 {
            self.finalize_image(width, height);
            return;
        }

        let peak_time_range = peaks.time_end - peaks.time_start;

        for px in 0..width {
            // What time does this pixel column represent in the viewport?
            let t = px as f64 / width as f64;
            let pixel_time = view.x_to_time(t);

            // Is this time within the peaks' time range?
            if pixel_time < peaks.time_start || pixel_time > peaks.time_end || peak_time_range <= 0.0 {
                continue;
            }

            // Map pixel time to peak index
            let peak_t = (pixel_time - peaks.time_start) / peak_time_range;
            let peak_idx = (peak_t * num_peaks as f64) as usize;
            if peak_idx >= num_peaks {
                continue;
            }

            let (min_val, max_val) = peaks.peaks[peak_idx];

            // Map -1..1 to pixel Y (inverted: top = positive)
            let y_max = (center_y as f32 - max_val * center_y as f32) as usize;
            let y_min = (center_y as f32 - min_val * center_y as f32) as usize;

            let y_top = y_max.min(y_min).min(height - 1);
            let y_bot = y_max.max(y_min).min(height - 1);

            for py in y_top..=y_bot {
                let idx = (py * width + px) * 3;
                if idx + 2 < self.cached_buffer.len() {
                    self.cached_buffer[idx] = WAVE_COLOR.0;
                    self.cached_buffer[idx + 1] = WAVE_COLOR.1;
                    self.cached_buffer[idx + 2] = WAVE_COLOR.2;
                }
            }
        }

        self.finalize_image(width, height);
    }

    fn finalize_image(&mut self, width: usize, height: usize) {
        match RgbImage::new(
            &self.cached_buffer,
            width as i32,
            height as i32,
            fltk::enums::ColorDepth::Rgb8,
        ) {
            Ok(img) => {
                self.cached_image = Some(img);
            }
            Err(e) => {
                eprintln!("Failed to create waveform image: {:?}", e);
                self.cached_image = None;
            }
        }
    }
}

impl Default for WaveformRenderer {
    fn default() -> Self {
        Self::new()
    }
}

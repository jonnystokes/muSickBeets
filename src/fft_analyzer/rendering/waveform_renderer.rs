use fltk::image::RgbImage;
use fltk::prelude::ImageExt;

use crate::data::ViewState;

/// Colors for the dark theme waveform
const BG_COLOR: (u8, u8, u8) = (0x1e, 0x1e, 0x2e);
const WAVE_COLOR: (u8, u8, u8) = (0x89, 0xb4, 0xfa);  // accent blue
const DOT_COLOR: (u8, u8, u8) = (0xf9, 0xe2, 0xaf);   // warm yellow for sample dots
const CENTER_LINE_COLOR: (u8, u8, u8) = (0x45, 0x47, 0x5a);
const CURSOR_COLOR: (u8, u8, u8) = (0xf3, 0x8b, 0xa8);  // red-pink

pub struct WaveformRenderer {
    cached_image: Option<RgbImage>,
    cached_buffer: Vec<u8>,
    cache_valid: bool,
    last_size: (i32, i32),
    last_view_hash: u64,
}

impl WaveformRenderer {
    pub fn new() -> Self {
        Self {
            cached_image: None,
            cached_buffer: Vec::new(),
            cache_valid: false,
            last_size: (0, 0),
            last_view_hash: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }

    fn view_hash(view: &ViewState, sample_count: usize, audio_time_start: f64, audio_time_end: f64) -> u64 {
        let mut h: u64 = 0;
        h = h.wrapping_mul(31).wrapping_add((view.time_min_sec * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((view.time_max_sec * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((audio_time_start * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add((audio_time_end * 10000.0) as u64);
        h = h.wrapping_mul(31).wrapping_add(sample_count as u64);
        h
    }

    /// Draw waveform from raw samples.
    /// `samples`: raw audio sample data
    /// `sample_rate`: audio sample rate
    /// `audio_time_start`: time offset of first sample in the full file timeline
    pub fn draw(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        audio_time_start: f64,
        view: &ViewState,
        cursor_x: Option<i32>,
        x: i32, y: i32, w: i32, h: i32,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }

        if samples.is_empty() {
            self.draw_no_data(x, y, w, h);
            return;
        }

        let audio_time_end = audio_time_start + samples.len() as f64 / sample_rate.max(1) as f64;
        let hash = Self::view_hash(view, samples.len(), audio_time_start, audio_time_end);
        let needs_rebuild = !self.cache_valid
            || self.last_size != (w, h)
            || self.last_view_hash != hash;

        if needs_rebuild {
            self.rebuild_cache(samples, sample_rate, audio_time_start, audio_time_end, view, w as usize, h as usize);
            self.last_size = (w, h);
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

    fn rebuild_cache(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        audio_time_start: f64,
        audio_time_end: f64,
        view: &ViewState,
        width: usize,
        height: usize,
    ) {
        let buffer_size = width * height * 3;
        if self.cached_buffer.len() != buffer_size {
            self.cached_buffer = vec![0u8; buffer_size];
        }

        let center_y = height / 2;
        let sr = sample_rate.max(1) as f64;
        let total_samples = samples.len();
        let audio_duration = audio_time_end - audio_time_start;

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

        if total_samples == 0 || audio_duration <= 0.0 {
            self.finalize_image(width, height);
            return;
        }

        // Calculate how many samples span one pixel in the viewport
        let view_duration = view.time_max_sec - view.time_min_sec;
        if view_duration <= 0.0 {
            self.finalize_image(width, height);
            return;
        }
        let seconds_per_pixel = view_duration / width as f64;
        let samples_per_pixel = seconds_per_pixel * sr;

        if samples_per_pixel > 4.0 {
            // Zoomed out: min/max peak rendering for each pixel column
            self.draw_peaks(samples, sr, audio_time_start, audio_time_end, view, width, height, center_y);
        } else {
            // Zoomed in: draw individual sample lines, with dots when very zoomed in
            let show_dots = samples_per_pixel < 0.33; // >3px per sample
            self.draw_samples(samples, sr, audio_time_start, audio_time_end, view, width, height, center_y, show_dots);
        }

        self.finalize_image(width, height);
    }

    /// Zoomed-out mode: compute min/max for each pixel column from raw samples
    fn draw_peaks(
        &mut self,
        samples: &[f32],
        sr: f64,
        audio_time_start: f64,
        audio_time_end: f64,
        view: &ViewState,
        width: usize,
        height: usize,
        center_y: usize,
    ) {
        let total_samples = samples.len();

        for px in 0..width {
            // Time range for this pixel column
            let t0 = px as f64 / width as f64;
            let t1 = (px + 1) as f64 / width as f64;
            let time0 = view.x_to_time(t0);
            let time1 = view.x_to_time(t1);

            // Check overlap with audio range
            if time1 < audio_time_start || time0 > audio_time_end {
                continue;
            }

            // Map to sample indices
            let s0 = ((time0 - audio_time_start) * sr).max(0.0) as usize;
            let s1 = (((time1 - audio_time_start) * sr).ceil() as usize).min(total_samples);

            if s0 >= s1 || s0 >= total_samples {
                continue;
            }

            let mut min_val = f32::MAX;
            let mut max_val = f32::MIN;
            for &s in &samples[s0..s1] {
                if s < min_val { min_val = s; }
                if s > max_val { max_val = s; }
            }

            // Map -1..1 to pixel Y (inverted: top = positive)
            let y_max = (center_y as f32 - max_val * center_y as f32) as usize;
            let y_min = (center_y as f32 - min_val * center_y as f32) as usize;

            let y_top = y_max.min(y_min).min(height - 1);
            let y_bot = y_max.max(y_min).min(height - 1);

            for py in y_top..=y_bot {
                self.set_pixel(px, py, width, WAVE_COLOR);
            }
        }
    }

    /// Zoomed-in mode: draw lines between individual samples, optionally with dots
    fn draw_samples(
        &mut self,
        samples: &[f32],
        sr: f64,
        audio_time_start: f64,
        audio_time_end: f64,
        view: &ViewState,
        width: usize,
        height: usize,
        center_y: usize,
        show_dots: bool,
    ) {
        let total_samples = samples.len();

        // Find the range of samples visible in the viewport (with margin for connecting lines)
        let view_start = view.time_min_sec;
        let view_end = view.time_max_sec;

        let first_sample = if view_start <= audio_time_start {
            0
        } else {
            ((view_start - audio_time_start) * sr).floor() as usize
        };
        let last_sample = if view_end >= audio_time_end {
            total_samples.saturating_sub(1)
        } else {
            (((view_end - audio_time_start) * sr).ceil() as usize).min(total_samples.saturating_sub(1))
        };

        // Add 1-sample margin on each side for connecting lines at edges
        let first_sample = first_sample.saturating_sub(1);
        let last_sample = (last_sample + 1).min(total_samples.saturating_sub(1));

        if first_sample >= total_samples {
            return;
        }

        // Convert sample index to pixel x position
        let sample_to_px = |idx: usize| -> f64 {
            let time = audio_time_start + idx as f64 / sr;
            let t = view.time_to_x(time);
            t * width as f64
        };

        // Convert sample value to pixel y position
        let val_to_py = |val: f32| -> i32 {
            (center_y as f32 - val.clamp(-1.0, 1.0) * center_y as f32) as i32
        };

        // Draw connecting lines between consecutive samples
        for i in first_sample..last_sample {
            let px0 = sample_to_px(i);
            let px1 = sample_to_px(i + 1);
            let py0 = val_to_py(samples[i]);
            let py1 = val_to_py(samples[i + 1]);

            self.draw_line(
                px0 as i32, py0, px1 as i32, py1,
                width, height, WAVE_COLOR,
            );
        }

        // Draw dots at sample positions when very zoomed in
        if show_dots {
            for i in first_sample..=last_sample {
                let px = sample_to_px(i);
                let py = val_to_py(samples[i]);
                let ipx = px.round() as i32;
                let ipy = py;

                // Draw a 3x3 dot
                for dy in -1..=1i32 {
                    for dx in -1..=1i32 {
                        let dx_pos = ipx + dx;
                        let dy_pos = ipy + dy;
                        if dx_pos >= 0 && (dx_pos as usize) < width && dy_pos >= 0 && (dy_pos as usize) < height {
                            self.set_pixel(dx_pos as usize, dy_pos as usize, width, DOT_COLOR);
                        }
                    }
                }
            }
        }
    }

    /// Bresenham's line algorithm for pixel buffer
    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, width: usize, height: usize, color: (u8, u8, u8)) {
        let mut x0 = x0;
        let mut y0 = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x0 >= 0 && (x0 as usize) < width && y0 >= 0 && (y0 as usize) < height {
                self.set_pixel(x0 as usize, y0 as usize, width, color);
            }
            if x0 == x1 && y0 == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy {
                if x0 == x1 { break; }
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                if y0 == y1 { break; }
                err += dx;
                y0 += sy;
            }
        }
    }

    #[inline]
    fn set_pixel(&mut self, x: usize, y: usize, width: usize, color: (u8, u8, u8)) {
        let idx = (y * width + x) * 3;
        if idx + 2 < self.cached_buffer.len() {
            self.cached_buffer[idx] = color.0;
            self.cached_buffer[idx + 1] = color.1;
            self.cached_buffer[idx + 2] = color.2;
        }
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

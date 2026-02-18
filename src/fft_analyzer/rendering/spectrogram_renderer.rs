use rayon::prelude::*;
use fltk::image::RgbImage;
use fltk::prelude::ImageExt;

use crate::data::{Spectrogram, ViewState};
use super::color_lut::ColorLUT;

pub struct SpectrogramRenderer {
    color_lut: ColorLUT,
    cached_image: Option<RgbImage>,
    cached_buffer: Vec<u8>,
    cache_valid: bool,
    last_widget_size: (i32, i32),
    last_view_hash: u64,
}

impl SpectrogramRenderer {
    pub fn new() -> Self {
        Self {
            color_lut: ColorLUT::default(),
            cached_image: None,
            cached_buffer: Vec::new(),
            cache_valid: false,
            last_widget_size: (0, 0),
            last_view_hash: 0,
        }
    }

    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }

    pub fn update_lut(&mut self, view: &ViewState) {
        if self.color_lut.set_params(view.threshold_db, view.db_ceiling, view.brightness, view.gamma, view.colormap) {
            self.cache_valid = false;
        }
        if self.color_lut.set_custom_stops(&view.custom_gradient) {
            self.cache_valid = false;
        }
    }

    fn view_hash(view: &ViewState, proc_time_min: f64, proc_time_max: f64, w: i32, h: i32) -> u64 {
        let mut hash: u64 = 0;
        hash = hash.wrapping_mul(31).wrapping_add((view.freq_min_hz * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.freq_max_hz * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.time_min_sec * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.time_max_sec * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add(match view.freq_scale {
            crate::data::FreqScale::Linear => 0,
            crate::data::FreqScale::Log => 1,
            crate::data::FreqScale::Power(p) => (p * 10000.0) as u64 + 2,
        });
        hash = hash.wrapping_mul(31).wrapping_add((view.threshold_db * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.db_ceiling * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.brightness * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.gamma * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add(view.colormap as u64);
        hash = hash.wrapping_mul(31).wrapping_add(w as u64);
        hash = hash.wrapping_mul(31).wrapping_add(h as u64);
        hash = hash.wrapping_mul(31).wrapping_add((proc_time_min * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((proc_time_max * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add(view.recon_freq_count as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.recon_freq_min_hz * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.recon_freq_max_hz * 100.0) as u64);
        // Include custom gradient in hash
        for stop in &view.custom_gradient {
            hash = hash.wrapping_mul(31).wrapping_add((stop.position * 10000.0) as u64);
            hash = hash.wrapping_mul(31).wrapping_add((stop.r * 255.0) as u64);
            hash = hash.wrapping_mul(31).wrapping_add((stop.g * 255.0) as u64);
            hash = hash.wrapping_mul(31).wrapping_add((stop.b * 255.0) as u64);
        }
        hash
    }

    fn needs_rebuild(&self, view: &ViewState, proc_time_min: f64, proc_time_max: f64, width: i32, height: i32) -> bool {
        if !self.cache_valid {
            return true;
        }
        let hash = Self::view_hash(view, proc_time_min, proc_time_max, width, height);
        hash != self.last_view_hash
    }

    /// Main draw method - call from widget's draw callback.
    /// proc_time_min/max: the processing time range (sidebar Start/Stop).
    /// Areas outside this time range are rendered grayed out.
    pub fn draw(
        &mut self,
        spec: &Spectrogram,
        view: &ViewState,
        proc_time_min: f64,
        proc_time_max: f64,
        x: i32, y: i32, w: i32, h: i32,
    ) {
        if w <= 0 || h <= 0 {
            return;
        }

        if spec.num_frames() == 0 || spec.num_bins() == 0 {
            self.draw_no_data(x, y, w, h);
            return;
        }

        if self.needs_rebuild(view, proc_time_min, proc_time_max, w, h) {
            self.update_lut(view);
            self.rebuild_cache(spec, view, proc_time_min, proc_time_max, w as usize, h as usize);
            self.last_widget_size = (w, h);
            self.last_view_hash = Self::view_hash(view, proc_time_min, proc_time_max, w, h);
            self.cache_valid = true;
        }

        if let Some(ref mut image) = self.cached_image {
            image.draw(x, y, w, h);
        }
    }

    fn draw_no_data(&self, x: i32, y: i32, w: i32, h: i32) {
        use fltk::draw;
        use fltk::enums::Color;
        draw::set_draw_color(Color::from_hex(0x1e1e2e));
        draw::draw_rectf(x, y, w, h);
        draw::set_draw_color(Color::from_hex(0x6c7086));
        draw::set_font(fltk::enums::Font::Helvetica, 14);
        draw::draw_text("Load an audio file to begin", x + 10, y + h / 2);
    }

    fn rebuild_cache(
        &mut self,
        spec: &Spectrogram,
        view: &ViewState,
        proc_time_min: f64,
        proc_time_max: f64,
        width: usize,
        height: usize,
    ) {
        let buffer_size = width * height * 3;
        if self.cached_buffer.len() != buffer_size {
            self.cached_buffer = vec![0u8; buffer_size];
        }

        let num_bins = spec.num_bins();

        // Pre-compute active bins per frame based on freq range + freq count filtering
        // This mirrors what the Reconstructor does, so the spectrogram shows
        // exactly which bins will be used for reconstruction.
        let freq_min = view.recon_freq_min_hz;
        let freq_max = view.recon_freq_max_hz;
        let freq_count = view.recon_freq_count;

        let active_bins: Vec<Vec<bool>> = spec.frames.par_iter()
            .map(|frame| {
                let mut bin_mags: Vec<(usize, f32)> = frame.magnitudes.iter()
                    .zip(frame.frequencies.iter())
                    .enumerate()
                    .filter_map(|(i, (&mag, &freq))| {
                        if freq >= freq_min && freq <= freq_max {
                            Some((i, mag))
                        } else {
                            None
                        }
                    })
                    .collect();
                bin_mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                let keep = freq_count.min(bin_mags.len());
                let mut active = vec![false; frame.magnitudes.len()];
                for &(idx, _) in &bin_mags[..keep] {
                    active[idx] = true;
                }
                active
            })
            .collect();

        // Pre-compute frequency bin for each pixel row (exact single bin, no blending)
        let row_bins: Vec<usize> = (0..height)
            .map(|py| {
                let flipped_py = height - 1 - py;
                let t = flipped_py as f32 / height as f32;
                let freq = view.y_to_freq(t);

                if let Some(first_frame) = spec.frames.first() {
                    // Find nearest bin by frequency
                    let mut best_bin = 0;
                    let mut best_dist = f32::MAX;
                    for (i, &f) in first_frame.frequencies.iter().enumerate() {
                        let dist = (f - freq).abs();
                        if dist < best_dist {
                            best_dist = dist;
                            best_bin = i;
                        }
                    }
                    best_bin.min(num_bins - 1)
                } else {
                    0
                }
            })
            .collect();

        // Pre-compute frame index and time for each pixel column (exact single frame)
        let col_data: Vec<(usize, f64)> = (0..width)
            .map(|px| {
                let t = px as f64 / width.max(1) as f64;
                let time = view.x_to_time(t);
                let frame_idx = spec.frame_at_time(time).unwrap_or(0);
                (frame_idx, time)
            })
            .collect();

        let lut = &self.color_lut;

        // Parallel rendering by rows
        let row_size = width * 3;
        self.cached_buffer
            .par_chunks_mut(row_size)
            .enumerate()
            .for_each(|(py, row)| {
                let bin = row_bins[py];

                for px in 0..width {
                    let (frame_idx, time) = col_data[px];

                    // Get exact magnitude for this single bin/frame
                    let max_mag = if let Some(frame) = spec.frames.get(frame_idx) {
                        if active_bins[frame_idx].get(bin).copied().unwrap_or(false) {
                            frame.magnitudes.get(bin).copied().unwrap_or(0.0)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    let (r, g, b) = lut.lookup(max_mag);

                    // Check if this pixel's time is within the processing range
                    let in_proc_range = time >= proc_time_min && time <= proc_time_max;

                    let idx = px * 3;
                    if in_proc_range {
                        row[idx] = r;
                        row[idx + 1] = g;
                        row[idx + 2] = b;
                    } else {
                        // Grayed out: desaturate and dim to ~35%
                        let gray = ((r as f32 * 0.3 + g as f32 * 0.59 + b as f32 * 0.11) * 0.35) as u8;
                        row[idx] = gray;
                        row[idx + 1] = gray;
                        row[idx + 2] = gray;
                    }
                }
            });

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
                eprintln!("Failed to create spectrogram image: {:?}", e);
                self.cached_image = None;
            }
        }
    }

}

impl Default for SpectrogramRenderer {
    fn default() -> Self {
        Self::new()
    }
}

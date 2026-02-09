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
        if self.color_lut.set_params(view.threshold_db, view.brightness, view.gamma, view.colormap) {
            self.cache_valid = false;
        }
    }

    fn view_hash(view: &ViewState, w: i32, h: i32) -> u64 {
        // Simple hash to detect view changes
        let mut hash: u64 = 0;
        hash = hash.wrapping_mul(31).wrapping_add((view.freq_min_hz * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.freq_max_hz * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.time_min_sec * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.time_max_sec * 10000.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add(view.freq_scale as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.threshold_db * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.brightness * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add((view.gamma * 100.0) as u64);
        hash = hash.wrapping_mul(31).wrapping_add(view.colormap as u64);
        hash = hash.wrapping_mul(31).wrapping_add(w as u64);
        hash = hash.wrapping_mul(31).wrapping_add(h as u64);
        hash
    }

    fn needs_rebuild(&self, view: &ViewState, width: i32, height: i32) -> bool {
        if !self.cache_valid {
            return true;
        }
        let hash = Self::view_hash(view, width, height);
        hash != self.last_view_hash
    }

    /// Main draw method - call from widget's draw callback
    pub fn draw(&mut self, spec: &Spectrogram, view: &ViewState, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }

        if spec.num_frames() == 0 || spec.num_bins() == 0 {
            self.draw_no_data(x, y, w, h);
            return;
        }

        if self.needs_rebuild(view, w, h) {
            self.update_lut(view);
            self.rebuild_cache(spec, view, w as usize, h as usize);
            self.last_widget_size = (w, h);
            self.last_view_hash = Self::view_hash(view, w, h);
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

    fn rebuild_cache(&mut self, spec: &Spectrogram, view: &ViewState, width: usize, height: usize) {
        let buffer_size = width * height * 3;
        if self.cached_buffer.len() != buffer_size {
            self.cached_buffer = vec![0u8; buffer_size];
        }

        let num_frames = spec.num_frames();
        let num_bins = spec.num_bins();

        // Pre-compute frequency bin for each pixel row
        let row_bins: Vec<(usize, usize)> = (0..height)
            .map(|py| {
                let flipped_py = height - 1 - py;
                let t = flipped_py as f32 / height as f32;
                let freq = view.y_to_freq(t);

                // Find the bin index for this frequency
                if let Some(first_frame) = spec.frames.first() {
                    let bin = first_frame.frequencies
                        .iter()
                        .position(|&f| f >= freq)
                        .unwrap_or(num_bins.saturating_sub(1));
                    let bin = bin.min(num_bins - 1);
                    // For pooling, use a small range around the bin
                    (bin, (bin + 1).min(num_bins))
                } else {
                    (0, 1)
                }
            })
            .collect();

        // Pre-compute frame index for each pixel column
        let col_frames: Vec<(usize, usize)> = (0..width)
            .map(|px| {
                let t = px as f64 / width as f64;
                let time = view.x_to_time(t);

                let frame_idx = spec.frame_at_time(time).unwrap_or(0);
                (frame_idx, (frame_idx + 1).min(num_frames))
            })
            .collect();

        let lut = &self.color_lut;

        // Parallel rendering by rows
        let row_size = width * 3;
        self.cached_buffer
            .par_chunks_mut(row_size)
            .enumerate()
            .for_each(|(py, row)| {
                let (bin_start, bin_end) = row_bins[py];

                for px in 0..width {
                    let (frame_start, frame_end) = col_frames[px];

                    // Get max magnitude in the region
                    let mut max_mag = 0.0f32;
                    for fi in frame_start..frame_end {
                        if let Some(frame) = spec.frames.get(fi) {
                            for bi in bin_start..bin_end {
                                if let Some(&mag) = frame.magnitudes.get(bi) {
                                    if mag > max_mag {
                                        max_mag = mag;
                                    }
                                }
                            }
                        }
                    }

                    let (r, g, b) = lut.lookup(max_mag);
                    let idx = px * 3;
                    row[idx] = r;
                    row[idx + 1] = g;
                    row[idx + 2] = b;
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

    pub fn is_cache_valid(&self) -> bool {
        self.cache_valid
    }
}

impl Default for SpectrogramRenderer {
    fn default() -> Self {
        Self::new()
    }
}

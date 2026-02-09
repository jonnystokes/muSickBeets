
/// Spectrogram Renderer with Image Buffer Caching
/// 
/// This module eliminates the massive draw call overhead by:
/// 1. Rendering to an RGB image buffer instead of individual rectangles
/// 2. Caching the image and only rebuilding when data/params change
/// 3. Proper scaling for both upsampling and downsampling
/// 4. Using pre-computed color LUT for fast lookups

use fltk::{
    draw,
    enums::Color,
    image::RgbImage,
    prelude::*,
};

use super::color_lut::ColorLUT;
use super::Spectrogram;

/// Pooling method for downsampling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoolingMethod {
    Max,      // Use maximum value in region (preserves peaks)
    Average,  // Average values in region (smoother)
    Nearest,  // Use nearest sample (fastest)
}

pub struct SpectrogramRenderer {
    color_lut: ColorLUT,
    cached_image: Option<RgbImage>,
    cached_buffer: Vec<u8>,
    cache_valid: bool,
    last_widget_size: (i32, i32),
    last_num_frames: usize,
    last_num_bins: usize,
    pooling_method: PoolingMethod,
}

impl SpectrogramRenderer {
    pub fn new() -> Self {
        Self {
            color_lut: ColorLUT::default(),
            cached_image: None,
            cached_buffer: Vec::new(),
            cache_valid: false,
            last_widget_size: (0, 0),
            last_num_frames: 0,
            last_num_bins: 0,
            pooling_method: PoolingMethod::Max,
        }
    }

    /// Invalidate cache - call when spectrogram data changes
    pub fn invalidate(&mut self) {
        self.cache_valid = false;
    }

    /// Set visualization parameters
    /// Returns true if parameters actually changed (and cache was invalidated)
    pub fn set_params(&mut self, threshold_db: f32, brightness: f32) -> bool {
        if self.color_lut.set_params(threshold_db, brightness) {
            self.cache_valid = false;
            true
        } else {
            false
        }
    }

    /// Set pooling method for downsampling
    pub fn set_pooling_method(&mut self, method: PoolingMethod) {
        if self.pooling_method != method {
            self.pooling_method = method;
            self.cache_valid = false;
        }
    }

    /// Check if cache needs rebuild
    fn needs_rebuild(&self, spec: &Spectrogram, width: i32, height: i32) -> bool {
        if !self.cache_valid {
            return true;
        }
        if self.last_widget_size != (width, height) {
            return true;
        }
        if self.last_num_frames != spec.num_frames() {
            return true;
        }
        if self.last_num_bins != spec.num_bins() {
            return true;
        }
        false
    }

    /// Main draw method - call from widget's draw callback
    pub fn draw(&mut self, spec: &Spectrogram, x: i32, y: i32, w: i32, h: i32) {
        // Skip if widget has no area
        if w <= 0 || h <= 0 {
            return;
        }

        // Skip if no data
        if spec.num_frames() == 0 || spec.num_bins() == 0 {
            self.draw_no_data(x, y, w, h);
            return;
        }

        // Rebuild cache if needed
        if self.needs_rebuild(spec, w, h) {
            self.rebuild_cache(spec, w as usize, h as usize);
            self.last_widget_size = (w, h);
            self.last_num_frames = spec.num_frames();
            self.last_num_bins = spec.num_bins();
            self.cache_valid = true;
        }

        // Draw cached image
        if let Some(ref mut image) = self.cached_image {
            image.draw(x, y, w, h);
        }
    }

    /// Draw "no data" message
    fn draw_no_data(&self, x: i32, y: i32, w: i32, h: i32) {
        draw::set_draw_color(Color::Black);
        draw::draw_rectf(x, y, w, h);
        draw::set_draw_color(Color::White);
        draw::set_font(fltk::enums::Font::Helvetica, 14);
        draw::draw_text("Load an audio file to begin", x + 10, y + h / 2);
    }

    /// Rebuild the image cache from spectrogram data
    fn rebuild_cache(&mut self, spec: &Spectrogram, width: usize, height: usize) {
        let num_frames = spec.num_frames();
        let num_bins = spec.num_bins();

        // Resize buffer if needed (RGB = 3 bytes per pixel)
        let buffer_size = width * height * 3;
        if self.cached_buffer.len() != buffer_size {
            self.cached_buffer = vec![0u8; buffer_size];
        }

        // Determine if we're upscaling or downscaling each axis
        let x_upscale = width > num_frames;
        let y_upscale = height > num_bins;

        // Render to buffer
        // Note: Y is flipped (low frequencies at bottom of display)
        for py in 0..height {
            // Map pixel Y to frequency bin (flip Y axis so low freq at bottom)
            let flipped_py = height - 1 - py;
            
            let (bin_start, bin_end) = if y_upscale {
                // Upscaling: use nearest neighbor
                let bin = (flipped_py * num_bins) / height;
                (bin, bin + 1)
            } else {
                // Downscaling: pool multiple bins
                let bin_start = (flipped_py * num_bins) / height;
                let bin_end = (((flipped_py + 1) * num_bins) / height).max(bin_start + 1);
                (bin_start, bin_end.min(num_bins))
            };

            for px in 0..width {
                let (frame_start, frame_end) = if x_upscale {
                    // Upscaling: use nearest neighbor
                    let frame = (px * num_frames) / width;
                    (frame, frame + 1)
                } else {
                    // Downscaling: pool multiple frames
                    let frame_start = (px * num_frames) / width;
                    let frame_end = (((px + 1) * num_frames) / width).max(frame_start + 1);
                    (frame_start, frame_end.min(num_frames))
                };

                // Get magnitude from the region
                let magnitude = self.get_magnitude_from_region(
                    spec,
                    frame_start, frame_end,
                    bin_start, bin_end,
                );

                // Look up color
                let (r, g, b) = self.color_lut.lookup(magnitude);

                // Write to buffer (RGB order)
                let idx = (py * width + px) * 3;
                self.cached_buffer[idx] = r;
                self.cached_buffer[idx + 1] = g;
                self.cached_buffer[idx + 2] = b;
            }
        }

        // Create FLTK image from buffer
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

    /// Get magnitude from a rectangular region of the spectrogram
    #[inline]
    fn get_magnitude_from_region(
        &self,
        spec: &Spectrogram,
        frame_start: usize,
        frame_end: usize,
        bin_start: usize,
        bin_end: usize,
    ) -> f32 {
        // Safety check
        if frame_start >= frame_end || bin_start >= bin_end {
            return 0.0;
        }

        match self.pooling_method {
            PoolingMethod::Nearest => {
                // Just use the first sample in the region
                if let Some(frame) = spec.frames.get(frame_start) {
                    if let Some(&mag) = frame.magnitudes.get(bin_start) {
                        return mag;
                    }
                }
                0.0
            }
            PoolingMethod::Max => {
                let mut max_mag = 0.0f32;
                for frame_idx in frame_start..frame_end {
                    if let Some(frame) = spec.frames.get(frame_idx) {
                        for bin_idx in bin_start..bin_end {
                            if let Some(&mag) = frame.magnitudes.get(bin_idx) {
                                if mag > max_mag {
                                    max_mag = mag;
                                }
                            }
                        }
                    }
                }
                max_mag
            }
            PoolingMethod::Average => {
                let mut sum = 0.0f32;
                let mut count = 0usize;
                for frame_idx in frame_start..frame_end {
                    if let Some(frame) = spec.frames.get(frame_idx) {
                        for bin_idx in bin_start..bin_end {
                            if let Some(&mag) = frame.magnitudes.get(bin_idx) {
                                sum += mag;
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    sum / count as f32
                } else {
                    0.0
                }
            }
        }
    }

    /// Get current threshold value
    pub fn threshold_db(&self) -> f32 {
        self.color_lut.threshold_db()
    }

    /// Get current brightness value
    pub fn brightness(&self) -> f32 {
        self.color_lut.brightness()
    }

    /// Check if cache is valid
    pub fn is_cache_valid(&self) -> bool {
        self.cache_valid
    }
}

impl Default for SpectrogramRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fft_analyzer::fft_engine::FftFrame;

    fn make_test_spectrogram(num_frames: usize, num_bins: usize) -> Spectrogram {
        let frames: Vec<FftFrame> = (0..num_frames)
            .map(|i| FftFrame {
                time_seconds: i as f64 * 0.01,
                frequencies: (0..num_bins).map(|f| f as f32 * 10.0).collect(),
                magnitudes: (0..num_bins).map(|f| (f as f32 / num_bins as f32) * 0.5).collect(),
                phases: vec![0.0; num_bins],
            })
            .collect();
        Spectrogram::from_frames(frames)
    }

    #[test]
    fn test_renderer_creation() {
        let renderer = SpectrogramRenderer::new();
        assert!(!renderer.is_cache_valid());
    }

    #[test]
    fn test_downscaling() {
        // More data than pixels
        let mut renderer = SpectrogramRenderer::new();
        let spec = make_test_spectrogram(1000, 512);
        
        renderer.rebuild_cache(&spec, 200, 100);
        
        assert!(renderer.cached_image.is_some());
        assert_eq!(renderer.cached_buffer.len(), 200 * 100 * 3);
    }

    #[test]
    fn test_upscaling() {
        // Fewer data than pixels - this was the bug case
        let mut renderer = SpectrogramRenderer::new();
        let spec = make_test_spectrogram(50, 32);
        
        renderer.rebuild_cache(&spec, 800, 400);
        
        assert!(renderer.cached_image.is_some());
        assert_eq!(renderer.cached_buffer.len(), 800 * 400 * 3);
        
        // Verify no zero pixels (stripes) in the middle
        let mid_idx = (200 * 800 + 400) * 3;
        let pixel = (
            renderer.cached_buffer[mid_idx],
            renderer.cached_buffer[mid_idx + 1],
            renderer.cached_buffer[mid_idx + 2],
        );
        // Should have some color, not black
        assert!(pixel.0 > 0 || pixel.1 > 0 || pixel.2 > 0);
    }

    #[test]
    fn test_extreme_upscaling() {
        // Very few frames, large display
        let mut renderer = SpectrogramRenderer::new();
        let spec = make_test_spectrogram(10, 8);
        
        renderer.rebuild_cache(&spec, 1000, 500);
        
        assert!(renderer.cached_image.is_some());
    }
}

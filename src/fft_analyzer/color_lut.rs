
/// Color Lookup Table for fast spectrogram rendering
/// Pre-computes all color values to avoid per-pixel math operations

const LUT_SIZE: usize = 1024;
const DB_MIN: f32 = -120.0;
const DB_MAX: f32 = 0.0;

#[derive(Clone)]
pub struct ColorLUT {
    table: Vec<(u8, u8, u8)>,
    threshold_db: f32,
    brightness: f32,
    db_range: f32,
}

impl ColorLUT {
    pub fn new(threshold_db: f32, brightness: f32) -> Self {
        let mut lut = Self {
            table: vec![(0, 0, 0); LUT_SIZE],
            threshold_db: threshold_db.clamp(-120.0, 0.0),
            brightness: brightness.clamp(0.1, 3.0),
            db_range: DB_MAX - DB_MIN,
        };
        lut.rebuild();
        lut
    }

    /// Rebuild the lookup table when parameters change
    pub fn rebuild(&mut self) {
        for i in 0..LUT_SIZE {
            // Map index to dB value
            let t = i as f32 / (LUT_SIZE - 1) as f32;
            let db = DB_MIN + t * self.db_range;
            
            // Calculate intensity with threshold
            let normalized = ((db - self.threshold_db) / (-self.threshold_db))
                .clamp(0.0, 1.0);
            
            // Apply gamma correction for brightness
            let intensity = normalized.powf(1.0 / self.brightness);
            
            // Convert to RGB
            self.table[i] = Self::intensity_to_rgb(intensity);
        }
    }

    /// Update parameters and rebuild if changed
    pub fn set_params(&mut self, threshold_db: f32, brightness: f32) -> bool {
        let new_threshold = threshold_db.clamp(-120.0, 0.0);
        let new_brightness = brightness.clamp(0.1, 3.0);
        
        if (new_threshold - self.threshold_db).abs() > 0.01 
            || (new_brightness - self.brightness).abs() > 0.01 
        {
            self.threshold_db = new_threshold;
            self.brightness = new_brightness;
            self.rebuild();
            true
        } else {
            false
        }
    }

    /// Fast lookup: magnitude -> RGB color
    #[inline(always)]
    pub fn lookup(&self, magnitude: f32) -> (u8, u8, u8) {
        // Convert magnitude to dB
        let db = 20.0 * magnitude.max(1e-10).log10();
        
        // Map dB to LUT index
        let t = (db - DB_MIN) / self.db_range;
        let index = (t * (LUT_SIZE - 1) as f32).clamp(0.0, (LUT_SIZE - 1) as f32) as usize;
        
        self.table[index]
    }

    /// Fast lookup from pre-computed dB value
    #[inline(always)]
    pub fn lookup_db(&self, db: f32) -> (u8, u8, u8) {
        let t = (db - DB_MIN) / self.db_range;
        let index = (t * (LUT_SIZE - 1) as f32).clamp(0.0, (LUT_SIZE - 1) as f32) as usize;
        self.table[index]
    }

    /// Colormap: blue -> cyan -> green -> yellow -> red
    fn intensity_to_rgb(intensity: f32) -> (u8, u8, u8) {
        let i = intensity.clamp(0.0, 1.0);
        
        if i < 0.25 {
            // Blue to Cyan
            let t = i * 4.0;
            (0, (t * 255.0) as u8, 255)
        } else if i < 0.5 {
            // Cyan to Green
            let t = (i - 0.25) * 4.0;
            (0, 255, ((1.0 - t) * 255.0) as u8)
        } else if i < 0.75 {
            // Green to Yellow
            let t = (i - 0.5) * 4.0;
            ((t * 255.0) as u8, 255, 0)
        } else {
            // Yellow to Red
            let t = (i - 0.75) * 4.0;
            (255, ((1.0 - t) * 255.0) as u8, 0)
        }
    }

    pub fn threshold_db(&self) -> f32 {
        self.threshold_db
    }

    pub fn brightness(&self) -> f32 {
        self.brightness
    }
}

impl Default for ColorLUT {
    fn default() -> Self {
        Self::new(-80.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lut_creation() {
        let lut = ColorLUT::new(-80.0, 1.0);
        assert_eq!(lut.table.len(), LUT_SIZE);
    }

    #[test]
    fn test_lut_lookup() {
        let lut = ColorLUT::new(-80.0, 1.0);
        
        // Very quiet (should be blue-ish)
        let (r, g, b) = lut.lookup(0.0001);
        assert!(b > r && b > g);
        
        // Loud (should be red-ish)
        let (r, g, b) = lut.lookup(1.0);
        assert!(r > g && r > b);
    }

    #[test]
    fn test_param_change_detection() {
        let mut lut = ColorLUT::new(-80.0, 1.0);
        
        // No change
        assert!(!lut.set_params(-80.0, 1.0));
        
        // Change threshold
        assert!(lut.set_params(-60.0, 1.0));
        
        // Change brightness
        assert!(lut.set_params(-60.0, 1.5));
    }
}


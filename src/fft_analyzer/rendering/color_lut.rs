use crate::data::{ColormapId, GradientStop, eval_gradient};

const LUT_SIZE: usize = 1024;

#[derive(Clone)]
pub struct ColorLUT {
    table: Vec<(u8, u8, u8)>,
    threshold_db: f32,
    db_ceiling: f32,
    brightness: f32,
    gamma: f32,
    colormap: ColormapId,
    custom_stops: Vec<GradientStop>,
}

impl ColorLUT {
    pub fn new(threshold_db: f32, db_ceiling: f32, brightness: f32, gamma: f32, colormap: ColormapId) -> Self {
        let mut lut = Self {
            table: vec![(0, 0, 0); LUT_SIZE],
            threshold_db: threshold_db.clamp(-200.0, 0.0),
            db_ceiling: db_ceiling.clamp(-200.0, 0.0),
            brightness: brightness.clamp(0.1, 3.0),
            gamma: gamma.clamp(0.1, 5.0),
            colormap,
            custom_stops: Vec::new(),
        };
        lut.rebuild();
        lut
    }

    /// Rebuild the LUT. Each entry maps a normalized t in [0,1] (from threshold
    /// to ceiling) through gamma correction and brightness to a colormap color.
    pub fn rebuild(&mut self) {
        for i in 0..LUT_SIZE {
            let t = i as f32 / (LUT_SIZE - 1) as f32;

            // Apply perceptual gamma correction then brightness
            let intensity = t.powf(1.0 / self.gamma) * self.brightness;
            let intensity = intensity.clamp(0.0, 1.0);

            self.table[i] = self.map_color(intensity);
        }
    }

    pub fn set_params(&mut self, threshold_db: f32, db_ceiling: f32, brightness: f32, gamma: f32, colormap: ColormapId) -> bool {
        let new_threshold = threshold_db.clamp(-200.0, 0.0);
        let new_ceiling = db_ceiling.clamp(-200.0, 0.0);
        let new_brightness = brightness.clamp(0.1, 3.0);
        let new_gamma = gamma.clamp(0.1, 5.0);

        if (new_threshold - self.threshold_db).abs() > 0.01
            || (new_ceiling - self.db_ceiling).abs() > 0.01
            || (new_brightness - self.brightness).abs() > 0.01
            || (new_gamma - self.gamma).abs() > 0.01
            || colormap != self.colormap
        {
            self.threshold_db = new_threshold;
            self.db_ceiling = new_ceiling;
            self.brightness = new_brightness;
            self.gamma = new_gamma;
            self.colormap = colormap;
            self.rebuild();
            true
        } else {
            false
        }
    }

    /// Update the custom gradient stops. Returns true if the LUT was rebuilt.
    pub fn set_custom_stops(&mut self, stops: &[GradientStop]) -> bool {
        if self.custom_stops.len() != stops.len()
            || self.custom_stops.iter().zip(stops.iter()).any(|(a, b)| a != b)
        {
            self.custom_stops = stops.to_vec();
            if self.colormap == ColormapId::Custom {
                self.rebuild();
                return true;
            }
        }
        false
    }

    /// Look up a color for a raw linear magnitude value.
    /// Converts magnitude to dB, normalizes to [threshold_db, db_ceiling] → [0,1],
    /// then indexes into the pre-built LUT.
    #[inline(always)]
    pub fn lookup(&self, magnitude: f32) -> (u8, u8, u8) {
        let db = 20.0 * magnitude.max(1e-10).log10();
        let range = self.db_ceiling - self.threshold_db;
        if range <= 0.0 {
            return self.table[0];
        }
        let t = (db - self.threshold_db) / range;
        let index = (t * (LUT_SIZE - 1) as f32).clamp(0.0, (LUT_SIZE - 1) as f32) as usize;
        self.table[index]
    }

    fn map_color(&self, intensity: f32) -> (u8, u8, u8) {
        match self.colormap {
            ColormapId::Classic => Self::colormap_classic(intensity),
            ColormapId::Viridis => Self::colormap_viridis(intensity),
            ColormapId::Magma => Self::colormap_magma(intensity),
            ColormapId::Inferno => Self::colormap_inferno(intensity),
            ColormapId::Greyscale => Self::colormap_greyscale(intensity),
            ColormapId::InvertedGrey => Self::colormap_inverted_grey(intensity),
            ColormapId::Geek => Self::colormap_geek(intensity),
            ColormapId::Custom => self.colormap_custom(intensity),
        }
    }

    fn colormap_custom(&self, t: f32) -> (u8, u8, u8) {
        let (r, g, b) = eval_gradient(&self.custom_stops, t.clamp(0.0, 1.0));
        (
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    /// SebLague-style 7-point gradient:
    /// Black → Dark Purple → Blue → Green → Yellow → Orange → Red
    /// with color stops at his exact positions (Unity ctime / 65535).
    fn colormap_classic(t: f32) -> (u8, u8, u8) {
        let t = t.clamp(0.0, 1.0);

        // SebLague gradient color keys with positions from Unity scene data:
        //   ctime 0     → 0.0000  Black     (0.00, 0.00, 0.00)
        //   ctime 17155 → 0.2618  Dk Purple (0.27, 0.11, 0.42)
        //   ctime 27178 → 0.4147  Blue      (0.17, 0.47, 0.92)
        //   ctime 42983 → 0.6559  Green     (0.34, 0.92, 0.22)
        //   ctime 49922 → 0.7618  Yellow    (0.88, 0.88, 0.12)
        //   ctime 57247 → 0.8735  Orange    (1.00, 0.56, 0.10)
        //   ctime 62644 → 0.9559  Red       (1.00, 0.00, 0.00)
        const STOPS: [(f32, f32, f32, f32); 7] = [
            (0.0000, 0.00, 0.00, 0.00), // Black
            (0.2618, 0.27, 0.11, 0.42), // Dark Purple
            (0.4147, 0.17, 0.47, 0.92), // Blue
            (0.6559, 0.34, 0.92, 0.22), // Green
            (0.7618, 0.88, 0.88, 0.12), // Yellow
            (0.8735, 1.00, 0.56, 0.10), // Orange
            (0.9559, 1.00, 0.00, 0.00), // Red
        ];

        // Find the two stops we're between
        let mut idx = 0;
        for i in 1..STOPS.len() {
            if t < STOPS[i].0 {
                break;
            }
            idx = i;
        }

        if idx >= STOPS.len() - 1 {
            // At or past last stop
            let s = &STOPS[STOPS.len() - 1];
            return (
                (s.1 * 255.0) as u8,
                (s.2 * 255.0) as u8,
                (s.3 * 255.0) as u8,
            );
        }

        let (pos0, r0, g0, b0) = STOPS[idx];
        let (pos1, r1, g1, b1) = STOPS[idx + 1];
        let seg_t = if (pos1 - pos0).abs() < 1e-6 {
            0.0
        } else {
            ((t - pos0) / (pos1 - pos0)).clamp(0.0, 1.0)
        };

        let r = r0 + (r1 - r0) * seg_t;
        let g = g0 + (g1 - g0) * seg_t;
        let b = b0 + (b1 - b0) * seg_t;

        (
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn colormap_viridis(t: f32) -> (u8, u8, u8) {
        // Approximate viridis: dark purple -> blue -> teal -> green -> yellow
        let t = t.clamp(0.0, 1.0);
        let r = ((-1.33 * t + 1.62) * t + 0.27) * t + 0.04;
        let g = ((0.57 * t - 1.30) * t + 1.42) * t + 0.01;
        let b = ((-2.40 * t + 2.26) * t - 0.15) * t + 0.33;
        (
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn colormap_magma(t: f32) -> (u8, u8, u8) {
        // Approximate magma: black -> dark purple -> red -> yellow -> white
        let t = t.clamp(0.0, 1.0);
        let r = ((-2.10 * t + 3.30) * t - 0.22) * t + 0.0;
        let g = ((-0.73 * t - 0.39) * t + 1.14) * t - 0.01;
        let b = ((0.69 * t - 2.49) * t + 2.13) * t + 0.16;
        (
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn colormap_inferno(t: f32) -> (u8, u8, u8) {
        // Approximate inferno: black -> purple -> red -> orange -> yellow
        let t = t.clamp(0.0, 1.0);
        let r = ((-1.83 * t + 2.96) * t + 0.03) * t + 0.0;
        let g = ((-0.84 * t + 0.03) * t + 0.82) * t - 0.01;
        let b = ((2.36 * t - 4.80) * t + 2.76) * t + 0.17;
        (
            (r.clamp(0.0, 1.0) * 255.0) as u8,
            (g.clamp(0.0, 1.0) * 255.0) as u8,
            (b.clamp(0.0, 1.0) * 255.0) as u8,
        )
    }

    fn colormap_greyscale(t: f32) -> (u8, u8, u8) {
        let v = (t.clamp(0.0, 1.0) * 255.0) as u8;
        (v, v, v)
    }

    fn colormap_inverted_grey(t: f32) -> (u8, u8, u8) {
        let v = ((1.0 - t.clamp(0.0, 1.0)) * 255.0) as u8;
        (v, v, v)
    }

    fn colormap_geek(t: f32) -> (u8, u8, u8) {
        // black -> dark green -> light green -> white at peak
        let t = t.clamp(0.0, 1.0);
        if t < 0.6 {
            // black (0,0,0) -> dark green (0,100,0)
            let s = t / 0.6;
            (0, (s * 100.0) as u8, 0)
        } else if t < 0.9 {
            // dark green (0,100,0) -> light green (144,238,144)
            let s = (t - 0.6) / 0.3;
            (
                (s * 144.0) as u8,
                (100.0 + s * 138.0) as u8,
                (s * 144.0) as u8,
            )
        } else {
            // light green (144,238,144) -> white (255,255,255)
            let s = (t - 0.9) / 0.1;
            (
                (144.0 + s * 111.0) as u8,
                (238.0 + s * 17.0) as u8,
                (144.0 + s * 111.0) as u8,
            )
        }
    }

}

impl Default for ColorLUT {
    fn default() -> Self {
        Self::new(-124.0, 0.0, 1.0, 2.2, ColormapId::Classic)
    }
}

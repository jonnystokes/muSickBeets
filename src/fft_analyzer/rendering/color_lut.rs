use crate::data::ColormapId;

const LUT_SIZE: usize = 1024;
const DB_MIN: f32 = -120.0;
const DB_MAX: f32 = 0.0;

#[derive(Clone)]
pub struct ColorLUT {
    table: Vec<(u8, u8, u8)>,
    threshold_db: f32,
    brightness: f32,
    gamma: f32,
    colormap: ColormapId,
    db_range: f32,
}

impl ColorLUT {
    pub fn new(threshold_db: f32, brightness: f32, gamma: f32, colormap: ColormapId) -> Self {
        let mut lut = Self {
            table: vec![(0, 0, 0); LUT_SIZE],
            threshold_db: threshold_db.clamp(-120.0, 0.0),
            brightness: brightness.clamp(0.1, 3.0),
            gamma: gamma.clamp(0.1, 5.0),
            colormap,
            db_range: DB_MAX - DB_MIN,
        };
        lut.rebuild();
        lut
    }

    pub fn rebuild(&mut self) {
        for i in 0..LUT_SIZE {
            let t = i as f32 / (LUT_SIZE - 1) as f32;
            let db = DB_MIN + t * self.db_range;

            let normalized = ((db - self.threshold_db) / (-self.threshold_db))
                .clamp(0.0, 1.0);

            // Apply perceptual gamma correction then brightness
            let intensity = normalized.powf(1.0 / self.gamma) * self.brightness;
            let intensity = intensity.clamp(0.0, 1.0);

            self.table[i] = self.map_color(intensity);
        }
    }

    pub fn set_params(&mut self, threshold_db: f32, brightness: f32, gamma: f32, colormap: ColormapId) -> bool {
        let new_threshold = threshold_db.clamp(-120.0, 0.0);
        let new_brightness = brightness.clamp(0.1, 3.0);
        let new_gamma = gamma.clamp(0.1, 5.0);

        if (new_threshold - self.threshold_db).abs() > 0.01
            || (new_brightness - self.brightness).abs() > 0.01
            || (new_gamma - self.gamma).abs() > 0.01
            || new_gamma != self.gamma
            || colormap != self.colormap
        {
            self.threshold_db = new_threshold;
            self.brightness = new_brightness;
            self.gamma = new_gamma;
            self.colormap = colormap;
            self.rebuild();
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn lookup(&self, magnitude: f32) -> (u8, u8, u8) {
        let db = 20.0 * magnitude.max(1e-10).log10();
        let t = (db - DB_MIN) / self.db_range;
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
        }
    }

    fn colormap_classic(i: f32) -> (u8, u8, u8) {
        let i = i.clamp(0.0, 1.0);
        if i < 0.25 {
            let t = i * 4.0;
            (0, (t * 255.0) as u8, 255)
        } else if i < 0.5 {
            let t = (i - 0.25) * 4.0;
            (0, 255, ((1.0 - t) * 255.0) as u8)
        } else if i < 0.75 {
            let t = (i - 0.5) * 4.0;
            ((t * 255.0) as u8, 255, 0)
        } else {
            let t = (i - 0.75) * 4.0;
            (255, ((1.0 - t) * 255.0) as u8, 0)
        }
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
        Self::new(-124.0, 1.0, 2.2, ColormapId::Viridis)
    }
}

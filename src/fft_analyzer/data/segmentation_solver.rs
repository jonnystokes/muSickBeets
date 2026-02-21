#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastEditedField {
    Overlap,
    SegmentsPerActive,
    BinsPerSegment,
}

#[derive(Debug, Clone, Copy)]
pub struct SolverConstraints {
    pub min_window: usize,
    pub max_window: usize,
    pub min_overlap_percent: f32,
    pub max_overlap_percent: f32,
}

impl Default for SolverConstraints {
    fn default() -> Self {
        Self {
            min_window: 2,
            max_window: 131072,
            min_overlap_percent: 0.0,
            max_overlap_percent: 95.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SolverInput {
    pub active_samples: usize,
    pub window_length: usize,
    pub overlap_percent: f32,
    pub zero_pad_factor: usize,
    pub target_segments_per_active: Option<usize>,
    pub target_bins_per_segment: Option<usize>,
    pub last_edited: LastEditedField,
    pub constraints: SolverConstraints,
}

#[derive(Debug, Clone, Copy)]
pub struct SolverOutput {
    pub window_length: usize,
    pub overlap_percent: f32,
    pub segments_per_active: usize,
    pub bins_per_segment: usize,
}

pub fn solve(input: SolverInput) -> SolverOutput {
    let constraints = input.constraints;
    let mut overlap = input.overlap_percent.clamp(
        constraints.min_overlap_percent,
        constraints.max_overlap_percent,
    );

    let mut window = clamp_even(
        input.window_length,
        constraints.min_window,
        constraints.max_window,
    );

    match input.last_edited {
        LastEditedField::SegmentsPerActive => {
            if let Some(target) = input.target_segments_per_active {
                window =
                    solve_window_for_segments(input.active_samples, overlap, target, constraints);
            }
        }
        LastEditedField::Overlap => {
            if let Some(target) = input.target_segments_per_active {
                window =
                    solve_window_for_segments(input.active_samples, overlap, target, constraints);
            }
        }
        LastEditedField::BinsPerSegment => {
            if let Some(target_bins) = input.target_bins_per_segment {
                let zpf = input.zero_pad_factor.max(1);
                let nfft = target_bins.saturating_sub(1).saturating_mul(2);
                let from_bins = (nfft / zpf).max(2);
                window = clamp_even(from_bins, constraints.min_window, constraints.max_window);
            }
        }
    }

    // Re-clamp overlap after any operation to keep deterministic caps stable.
    overlap = overlap.clamp(
        constraints.min_overlap_percent,
        constraints.max_overlap_percent,
    );

    let hop = hop_length(window, overlap);
    let segments = num_segments(input.active_samples, window, hop);
    let bins = (window * input.zero_pad_factor.max(1)) / 2 + 1;

    SolverOutput {
        window_length: window,
        overlap_percent: overlap,
        segments_per_active: segments,
        bins_per_segment: bins,
    }
}

fn solve_window_for_segments(
    active_samples: usize,
    overlap_percent: f32,
    target_segments: usize,
    constraints: SolverConstraints,
) -> usize {
    let target = target_segments.max(1);
    let overlap_ratio = overlap_percent.clamp(0.0, 95.0) / 100.0;
    let hop_factor = (1.0 - overlap_ratio).max(0.01);

    let approx_window = if target <= 1 {
        active_samples.max(2)
    } else {
        let denom = 1.0 + (target - 1) as f32 * hop_factor;
        ((active_samples as f32) / denom).round() as usize
    };

    let approx_window = clamp_even(
        approx_window,
        constraints.min_window,
        constraints.max_window,
    );

    // Deterministic local search around approximation for closest segment count.
    let mut best = approx_window;
    let mut best_err = usize::MAX;
    let mut best_dist = usize::MAX;

    for step in 0..256usize {
        let candidates = if step == 0 {
            [approx_window, approx_window]
        } else {
            [
                approx_window.saturating_sub(step * 2),
                approx_window.saturating_add(step * 2),
            ]
        };

        for mut cand in candidates {
            cand = clamp_even(cand, constraints.min_window, constraints.max_window);
            let hop = hop_length(cand, overlap_percent);
            let segs = num_segments(active_samples, cand, hop);
            let err = segs.abs_diff(target);
            let dist = cand.abs_diff(approx_window);
            if err < best_err
                || (err == best_err && dist < best_dist)
                || (err == best_err && dist == best_dist && cand < best)
            {
                best = cand;
                best_err = err;
                best_dist = dist;
            }
            if best_err == 0 {
                return best;
            }
        }
    }

    best
}

fn clamp_even(value: usize, min: usize, max: usize) -> usize {
    let mut v = value.clamp(min.max(2), max.max(2));
    if v % 2 != 0 {
        v = if v == max { v.saturating_sub(1) } else { v + 1 };
    }
    v.max(2)
}

fn hop_length(window: usize, overlap_percent: f32) -> usize {
    ((window as f32) * (1.0 - (overlap_percent / 100.0))).max(1.0) as usize
}

fn num_segments(active_samples: usize, window: usize, hop: usize) -> usize {
    if active_samples < window {
        return 0;
    }
    (active_samples.saturating_sub(window)) / hop.max(1) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_edit_prefers_overlap_and_solves_window() {
        let out = solve(SolverInput {
            active_samples: 44_100,
            window_length: 8192,
            overlap_percent: 75.0,
            zero_pad_factor: 1,
            target_segments_per_active: Some(18),
            target_bins_per_segment: None,
            last_edited: LastEditedField::SegmentsPerActive,
            constraints: SolverConstraints::default(),
        });
        assert_eq!(out.overlap_percent, 75.0);
        assert_eq!(out.segments_per_active, 18);
    }

    #[test]
    fn overlap_edit_uses_locked_segments_when_present() {
        let out = solve(SolverInput {
            active_samples: 44_100,
            window_length: 8192,
            overlap_percent: 50.0,
            zero_pad_factor: 1,
            target_segments_per_active: Some(10),
            target_bins_per_segment: None,
            last_edited: LastEditedField::Overlap,
            constraints: SolverConstraints::default(),
        });
        assert_eq!(out.segments_per_active, 10);
    }

    #[test]
    fn bins_edit_updates_window_deterministically() {
        let out = solve(SolverInput {
            active_samples: 44_100,
            window_length: 8192,
            overlap_percent: 75.0,
            zero_pad_factor: 1,
            target_segments_per_active: Some(18),
            target_bins_per_segment: Some(1025),
            last_edited: LastEditedField::BinsPerSegment,
            constraints: SolverConstraints::default(),
        });
        assert_eq!(out.window_length, 2048);
        assert_eq!(out.bins_per_segment, 1025);
    }

    #[test]
    fn bins_edit_respects_zero_pad_factor_mapping() {
        let out = solve(SolverInput {
            active_samples: 44_100,
            window_length: 8192,
            overlap_percent: 75.0,
            zero_pad_factor: 4,
            target_segments_per_active: None,
            target_bins_per_segment: Some(1025),
            last_edited: LastEditedField::BinsPerSegment,
            constraints: SolverConstraints::default(),
        });

        // 1025 bins -> nfft 2048; with 4x pad, base window should be 512.
        assert_eq!(out.window_length, 512);
        assert_eq!(out.bins_per_segment, 1025);
    }

    #[test]
    fn solver_enforces_hard_constraints() {
        let out = solve(SolverInput {
            active_samples: 1_000_000,
            window_length: 999_999,
            overlap_percent: 123.0,
            zero_pad_factor: 1,
            target_segments_per_active: Some(1),
            target_bins_per_segment: None,
            last_edited: LastEditedField::SegmentsPerActive,
            constraints: SolverConstraints::default(),
        });

        assert_eq!(out.window_length, 131072);
        assert!(out.window_length % 2 == 0);
        assert_eq!(out.overlap_percent, 95.0);
        assert_eq!(out.bins_per_segment, 65_537);
    }

    #[test]
    fn bins_edit_respects_min_even_window_floor() {
        let out = solve(SolverInput {
            active_samples: 44_100,
            window_length: 8,
            overlap_percent: 75.0,
            zero_pad_factor: 8,
            target_segments_per_active: None,
            target_bins_per_segment: Some(2),
            last_edited: LastEditedField::BinsPerSegment,
            constraints: SolverConstraints::default(),
        });

        // 2 bins -> nfft 2; with 8x pad this maps below base min and must clamp to 2.
        assert_eq!(out.window_length, 2);
        assert_eq!(out.bins_per_segment, 9);
    }
}

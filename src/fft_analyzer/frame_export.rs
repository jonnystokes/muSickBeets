use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::data::spectrogram::compute_active_bins;
use crate::data::{FftParams, Spectrogram, ViewState, WindowType};

/// Export a single selected FFT frame as an instrument fingerprint file.
///
/// Only **active bins** (those passing the current frequency range and top-N
/// filters) are written. The future instrument program receives exactly the
/// shaped fingerprint the analyzer intended — no second-stage bin selection
/// needed.
///
/// File format (CSV-like, versioned):
///   - Line 1: `MUSICKBEETS_FRAME_V1`
///   - Lines 2–N: `key=value` metadata, one per line
///   - Separator line: `---`
///   - Header line: `bin_index,frequency_hz,magnitude,phase_rad`
///   - Data lines: one per active bin
pub fn export_single_frame<P: AsRef<Path>>(
    path: P,
    spectrogram: &Spectrogram,
    frame_index: usize,
    params: &FftParams,
    view: &ViewState,
) -> Result<usize> {
    let frame = spectrogram
        .frames
        .get(frame_index)
        .context("Frame index out of range")?;

    // Compute active bins using the same logic as reconstruction/rendering
    let active = compute_active_bins(
        &frame.magnitudes,
        &spectrogram.frequencies,
        view.recon_freq_min_hz,
        view.recon_freq_max_hz,
        view.recon_freq_count,
    );

    let window_type_str = match params.window_type {
        WindowType::Rectangular => "Rectangular".to_string(),
        WindowType::Hann => "Hann".to_string(),
        WindowType::Hamming => "Hamming".to_string(),
        WindowType::Blackman => "Blackman".to_string(),
        WindowType::Kaiser(beta) => format!("Kaiser_{}", beta),
    };

    let file = File::create(&path)
        .with_context(|| format!("Failed to create frame file: {:?}", path.as_ref()))?;
    let mut w = std::io::BufWriter::new(file);

    // ── Magic + metadata header ──
    writeln!(w, "MUSICKBEETS_FRAME_V1")?;
    writeln!(w, "sample_rate={}", params.sample_rate)?;
    writeln!(w, "fft_size={}", params.window_length)?;
    writeln!(w, "hop_length={}", params.hop_length())?;
    writeln!(w, "overlap_percent={}", params.overlap_percent)?;
    writeln!(w, "window_type={}", window_type_str)?;
    writeln!(w, "zero_pad_factor={}", params.zero_pad_factor)?;
    writeln!(
        w,
        "centered={}",
        if params.use_center { "true" } else { "false" }
    )?;
    writeln!(w, "spectrum_type=onesided")?;
    writeln!(w, "total_bin_count={}", spectrogram.frequencies.len())?;
    writeln!(w, "nyquist_hz={:.2}", params.sample_rate as f32 / 2.0)?;
    writeln!(
        w,
        "frequency_resolution_hz={:.6}",
        params.sample_rate as f64 / params.window_length as f64
    )?;
    writeln!(w, "frame_index={}", frame_index)?;
    writeln!(w, "frame_time_seconds={:.10}", frame.time_seconds)?;
    writeln!(w, "recon_freq_min_hz={:.2}", view.recon_freq_min_hz)?;
    writeln!(w, "recon_freq_max_hz={:.2}", view.recon_freq_max_hz)?;
    writeln!(w, "recon_freq_count={}", view.recon_freq_count)?;
    writeln!(w, "recon_norm_floor={}", view.recon_norm_floor)?;
    writeln!(w, "magnitude_unit=linear")?;
    writeln!(w, "phase_unit=radians")?;
    writeln!(
        w,
        "phase_reference={}",
        if params.use_center {
            "frame_center"
        } else {
            "frame_start"
        }
    )?;
    writeln!(w, "scaling=raw_fft_coefficient")?;

    // Count active bins
    let active_count = active.iter().filter(|&&b| b).count();
    writeln!(w, "active_bin_count={}", active_count)?;

    // ── Separator + data ──
    writeln!(w, "---")?;
    writeln!(w, "bin_index,frequency_hz,magnitude,phase_rad")?;

    for (i, &is_active) in active.iter().enumerate() {
        if !is_active {
            continue;
        }
        let freq = spectrogram.frequencies.get(i).copied().unwrap_or(0.0);
        let mag = frame.magnitudes.get(i).copied().unwrap_or(0.0);
        let phase = frame.phases.get(i).copied().unwrap_or(0.0);
        writeln!(w, "{},{:.4},{:.6},{:.6}", i, freq, mag, phase)?;
    }

    w.flush().context("Failed to flush frame export")?;

    dbg_log!(
        crate::debug_flags::FILE_IO_DBG,
        "FrameExport",
        "Exported frame {} ({:.5}s): {} active bins of {} total to {:?}",
        frame_index,
        frame.time_seconds,
        active_count,
        spectrogram.frequencies.len(),
        path.as_ref()
    );

    Ok(active_count)
}

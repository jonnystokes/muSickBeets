use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;

use super::data::{
    FftFrame, FftParams, LastEditedField, Spectrogram, TimeUnit, ViewState, WindowType,
};

/// Reconstruction parameters imported from CSV: (freq_count, freq_min_hz, freq_max_hz).
pub type ReconParams = (usize, f32, f32);

/// Viewport display state imported from CSV.
#[derive(Debug, Clone)]
pub struct ImportedViewParams {
    /// Viewport frequency display range (what the user was looking at when saving).
    pub freq_min_hz: Option<f32>,
    pub freq_max_hz: Option<f32>,
}

/// Export spectrogram to CSV, optionally filtering to a time range.
///
/// If `time_range` is `Some((min, max))`, only frames within the range are written.
/// Pass `None` to export all frames.
pub fn export_to_csv<P: AsRef<Path>>(
    spectrogram: &Spectrogram,
    params: &FftParams,
    view: &ViewState,
    path: P,
    time_range: Option<(f64, f64)>,
) -> Result<()> {
    let file = File::create(&path)
        .with_context(|| format!("Failed to create CSV file: {:?}", path.as_ref()))?;

    let mut writer = csv::WriterBuilder::new()
        .flexible(true) // Allow rows with different numbers of fields
        .from_writer(file);

    // Write metadata header (row 1): FFT params + reconstruction params
    let window_type_str = match params.window_type {
        WindowType::Hann => "Hann".to_string(),
        WindowType::Hamming => "Hamming".to_string(),
        WindowType::Blackman => "Blackman".to_string(),
        WindowType::Kaiser(beta) => format!("Kaiser_{}", beta),
    };

    writer
        .write_record(&[
            params.sample_rate.to_string(),                             // 0
            params.window_length.to_string(),                           // 1
            params.hop_length().to_string(),                            // 2
            params.overlap_percent.to_string(),                         // 3
            window_type_str,                                            // 4
            params.use_center.to_string(),                              // 5
            "1".to_string(),                                            // 6: num_channels
            params.start_sample.to_string(),                            // 7
            params.stop_sample.to_string(),                             // 8
            view.recon_freq_count.to_string(),                          // 9
            format!("{:.2}", view.recon_freq_min_hz),                   // 10
            format!("{:.2}", view.recon_freq_max_hz),                   // 11
            params.zero_pad_factor.to_string(),                         // 12
            params.target_segments_per_active.unwrap_or(0).to_string(), // 13
            params.target_bins_per_segment.unwrap_or(0).to_string(),    // 14
            match params.last_edited_field {
                LastEditedField::Overlap => "Overlap".to_string(),
                LastEditedField::SegmentsPerActive => "SegmentsPerActive".to_string(),
                LastEditedField::BinsPerSegment => "BinsPerSegment".to_string(),
            }, // 15
            format!("{:.2}", view.freq_min_hz),                         // 16: viewport freq min
            format!("{:.2}", view.freq_max_hz),                         // 17: viewport freq max
        ])
        .context("Failed to write CSV metadata")?;

    // Write column labels (row 2)
    writer
        .write_record(["time_sec", "frequency_hz", "magnitude", "phase_rad"])
        .context("Failed to write CSV header")?;

    // Write data (row 3+)
    let freqs = &spectrogram.frequencies;
    for frame in &spectrogram.frames {
        // Skip frames outside the time range if specified
        if let Some((t_min, t_max)) = time_range
            && (frame.time_seconds < t_min || frame.time_seconds > t_max)
        {
            continue;
        }
        let time = frame.time_seconds;

        for (i, &freq) in freqs.iter().enumerate() {
            writer
                .write_record(&[
                    format!("{:.10}", time),
                    format!("{:.4}", freq),
                    format!("{:.6}", frame.magnitudes[i]),
                    format!("{:.6}", frame.phases[i]),
                ])
                .context("Failed to write CSV record")?;
        }
    }

    writer.flush().context("Failed to flush CSV writer")?;

    Ok(())
}

/// Returns (Spectrogram, FftParams, optional recon params, viewport params)
pub fn import_from_csv<P: AsRef<Path>>(
    path: P,
) -> Result<(Spectrogram, FftParams, Option<ReconParams>, ImportedViewParams)> {
    use csv::ReaderBuilder;

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true) // Allow rows with different numbers of fields
        .from_path(&path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path.as_ref()))?;

    let mut records = reader.records();

    // Read metadata (row 1)
    let metadata = records
        .next()
        .ok_or_else(|| anyhow::anyhow!("CSV file is empty"))?
        .context("Failed to read metadata row")?;

    if metadata.len() < 9 {
        anyhow::bail!(
            "Invalid metadata row - expected at least 9 fields, got {}",
            metadata.len()
        );
    }

    let sample_rate: u32 = metadata[0]
        .parse()
        .context("Invalid sample_rate in metadata")?;
    let window_length: usize = metadata[1]
        .parse()
        .context("Invalid window_length in metadata")?;
    let _hop_length: usize = metadata[2]
        .parse()
        .context("Invalid hop_length in metadata")?;
    let overlap_percent: f32 = metadata[3]
        .parse()
        .context("Invalid overlap_percent in metadata")?;

    let window_type = if metadata[4].starts_with("Kaiser_") {
        let beta: f32 = metadata[4]
            .trim_start_matches("Kaiser_")
            .parse()
            .context("Invalid Kaiser beta in metadata")?;
        WindowType::Kaiser(beta)
    } else {
        match metadata[4].as_ref() {
            "Hann" => WindowType::Hann,
            "Hamming" => WindowType::Hamming,
            "Blackman" => WindowType::Blackman,
            _ => anyhow::bail!("Unknown window type: {}", &metadata[4]),
        }
    };

    let use_center: bool = metadata[5]
        .parse()
        .context("Invalid use_center in metadata")?;
    let start_sample: usize = metadata[7]
        .parse()
        .context("Invalid start_sample in metadata")?;
    let stop_sample: usize = metadata[8]
        .parse()
        .context("Invalid stop_sample in metadata")?;

    // Read optional reconstruction params (fields 9-11, backward-compatible)
    let recon_params = if metadata.len() >= 12 {
        let freq_count: usize = metadata[9].parse().unwrap_or(4097);
        let freq_min: f32 = metadata[10].parse().unwrap_or(0.0);
        let freq_max: f32 = metadata[11].parse().unwrap_or(5000.0);
        Some((freq_count, freq_min, freq_max))
    } else {
        None
    };

    // Read optional zero_pad_factor (field 12, backward-compatible)
    let zero_pad_factor: usize = if metadata.len() >= 13 {
        metadata[12].parse().unwrap_or(1)
    } else {
        1
    };

    // Optional segmentation solver metadata (fields 13-15, backward-compatible)
    let target_segments_per_active: Option<usize> = if metadata.len() >= 14 {
        let n = metadata[13].parse().unwrap_or(0);
        if n > 0 {
            Some(n)
        } else {
            None
        }
    } else {
        None
    };

    let target_bins_per_segment: Option<usize> = if metadata.len() >= 15 {
        let n = metadata[14].parse().unwrap_or(0);
        if n > 0 {
            Some(n)
        } else {
            None
        }
    } else {
        None
    };

    let last_edited_field = if metadata.len() >= 16 {
        match metadata[15].as_ref() {
            "SegmentsPerActive" => LastEditedField::SegmentsPerActive,
            "BinsPerSegment" => LastEditedField::BinsPerSegment,
            _ => LastEditedField::Overlap,
        }
    } else {
        LastEditedField::Overlap
    };

    // Optional viewport frequency range (fields 16-17, backward-compatible)
    let view_params = ImportedViewParams {
        freq_min_hz: if metadata.len() >= 17 {
            metadata[16].parse().ok()
        } else {
            None
        },
        freq_max_hz: if metadata.len() >= 18 {
            metadata[17].parse().ok()
        } else {
            None
        },
    };

    // Skip column labels (row 2) — validate it exists and looks like a header
    match records.next() {
        Some(Ok(row)) => {
            // Sanity check: first field should be the column label, not numeric data
            if let Some(first) = row.get(0)
                && first.parse::<f64>().is_ok() {
                    eprintln!("[CSV Import] Warning: row 2 looks like data, not a header (first field: {:?}). It will be skipped.", first);
                }
        }
        Some(Err(e)) => {
            eprintln!(
                "[CSV Import] Warning: failed to read row 2 (column labels): {}",
                e
            );
        }
        None => {
            anyhow::bail!("CSV file has no data rows (only metadata header)");
        }
    }

    // Read data rows
    let mut frames_map: std::collections::BTreeMap<String, Vec<(f32, f32, f32)>> =
        std::collections::BTreeMap::new();

    for result in records {
        let record = result.context("Failed to read CSV record")?;

        if record.len() < 4 {
            continue;
        }

        let time_sec: String = record[0].to_string();
        let frequency_hz: f32 = record[1].parse().unwrap_or(0.0);
        let magnitude: f32 = record[2].parse().unwrap_or(0.0);
        let phase_rad: f32 = record[3].parse().unwrap_or(0.0);

        frames_map.entry(time_sec).or_default().push((
            frequency_hz,
            magnitude,
            phase_rad,
        ));
    }

    // Build frames. Frequency bins are shared across all frames (stored once
    // on Spectrogram), so we extract them from the first frame's data.
    let mut frames = Vec::new();
    let mut shared_frequencies: Option<Vec<f32>> = None;
    for (time_str, bins) in frames_map {
        let time_seconds: f64 = time_str.parse().unwrap_or(0.0);

        let mut magnitudes = Vec::new();
        let mut phases = Vec::new();

        if shared_frequencies.is_none() {
            // First frame: extract frequency values for the shared vector
            let freqs: Vec<f32> = bins.iter().map(|&(freq, _, _)| freq).collect();
            shared_frequencies = Some(freqs);
        }

        for (_freq, mag, phase) in bins {
            magnitudes.push(mag);
            phases.push(phase);
        }

        frames.push(FftFrame {
            time_seconds,
            magnitudes,
            phases,
        });
    }

    let spectrogram = Spectrogram::from_frames_with_frequencies(
        frames,
        shared_frequencies.unwrap_or_default(),
    );

    let params = FftParams {
        window_length,
        overlap_percent,
        window_type,
        use_center,
        start_sample,
        stop_sample,
        time_unit: TimeUnit::Seconds,
        sample_rate,
        zero_pad_factor,
        target_segments_per_active,
        target_bins_per_segment,
        last_edited_field,
    };

    Ok((spectrogram, params, recon_params, view_params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_roundtrip() {
        let frame = FftFrame {
            time_seconds: 0.0,
            magnitudes: vec![0.5, 0.8, 0.3],
            phases: vec![0.0, 1.57, 3.14],
        };

        let spec = Spectrogram::from_frames_with_frequencies(
            vec![frame],
            vec![0.0, 100.0, 200.0],
        );
        let mut params = FftParams::default();
        params.target_segments_per_active = Some(17);
        params.target_bins_per_segment = Some(1025);
        params.last_edited_field = LastEditedField::SegmentsPerActive;
        let view = ViewState::default();

        let temp_path = "/tmp/test_roundtrip.csv";
        export_to_csv(&spec, &params, &view, temp_path, None).expect("Export should succeed");

        let (imported_spec, imported_params, recon, view_imported) =
            import_from_csv(temp_path).expect("Import should succeed");

        assert_eq!(imported_params.sample_rate, params.sample_rate);
        assert_eq!(imported_params.window_length, params.window_length);
        assert_eq!(
            imported_params.target_segments_per_active,
            params.target_segments_per_active
        );
        assert_eq!(
            imported_params.target_bins_per_segment,
            params.target_bins_per_segment
        );
        assert_eq!(imported_params.last_edited_field, params.last_edited_field);
        assert_eq!(imported_spec.num_frames(), 1);
        assert!(recon.is_some());
        let (fc, fmin, fmax) = recon.unwrap();
        assert_eq!(fc, view.recon_freq_count);
        assert!((fmin - view.recon_freq_min_hz).abs() < 1.0);
        assert!((fmax - view.recon_freq_max_hz).abs() < 1.0);

        // Viewport freq range roundtrip
        assert!(view_imported.freq_min_hz.is_some());
        assert!(view_imported.freq_max_hz.is_some());
        assert!((view_imported.freq_min_hz.unwrap() - view.freq_min_hz).abs() < 1.0);
        assert!((view_imported.freq_max_hz.unwrap() - view.freq_max_hz).abs() < 1.0);

        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_csv_import_backward_compat_without_solver_fields() {
        let temp_path = "/tmp/test_backward_compat.csv";
        let csv = "48000,8192,2048,75,Hann,false,1,0,44100,4097,0.00,5000.00,1
"
        .to_string()
            + "time_sec,frequency_hz,magnitude,phase_rad
" + "0.00000,0.0000,0.500000,0.000000
";
        std::fs::write(temp_path, csv).expect("write test csv");

        let (_spec, params, _recon, _view) = import_from_csv(temp_path).expect("import should succeed");
        assert_eq!(params.target_segments_per_active, None);
        assert_eq!(params.target_bins_per_segment, None);
        assert_eq!(params.last_edited_field, LastEditedField::Overlap);

        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_csv_multiple_frames() {
        let frames = vec![
            FftFrame {
                time_seconds: 0.0,
                magnitudes: vec![0.1, 0.2],
                phases: vec![0.0, 0.5],
            },
            FftFrame {
                time_seconds: 0.01,
                magnitudes: vec![0.3, 0.4],
                phases: vec![1.0, 1.5],
            },
        ];

        let spec = Spectrogram::from_frames_with_frequencies(
            frames,
            vec![0.0, 100.0],
        );
        let params = FftParams::default();
        let view = ViewState::default();

        let temp_path = "/tmp/test_multi_frames.csv";
        export_to_csv(&spec, &params, &view, temp_path, None).expect("Export should succeed");

        let (imported_spec, _, _, _) = import_from_csv(temp_path).expect("Import should succeed");
        assert_eq!(imported_spec.num_frames(), 2);

        std::fs::remove_file(temp_path).ok();
    }
}

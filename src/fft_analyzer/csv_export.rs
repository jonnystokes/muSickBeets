
use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;

use super::data::{Spectrogram, FftParams, FftFrame, ViewState, WindowType, TimeUnit};

pub fn export_to_csv<P: AsRef<Path>>(spectrogram: &Spectrogram, params: &FftParams, view: &ViewState, path: P) -> Result<()> {
    let file = File::create(&path)
        .with_context(|| format!("Failed to create CSV file: {:?}", path.as_ref()))?;

    let mut writer = csv::WriterBuilder::new()
        .flexible(true)  // Allow rows with different numbers of fields
        .from_writer(file);

    // Write metadata header (row 1): FFT params + reconstruction params
    let window_type_str = match params.window_type {
        WindowType::Hann => "Hann".to_string(),
        WindowType::Hamming => "Hamming".to_string(),
        WindowType::Blackman => "Blackman".to_string(),
        WindowType::Kaiser(beta) => format!("Kaiser_{}", beta),
    };

    writer.write_record(&[
        params.sample_rate.to_string(),            // 0
        params.window_length.to_string(),           // 1
        params.hop_length().to_string(),            // 2
        params.overlap_percent.to_string(),         // 3
        window_type_str,                            // 4
        params.use_center.to_string(),              // 5
        "1".to_string(),                            // 6: num_channels
        params.start_sample.to_string(),           // 7
        params.stop_sample.to_string(),            // 8
        view.recon_freq_count.to_string(),          // 9
        format!("{:.2}", view.recon_freq_min_hz),   // 10
        format!("{:.2}", view.recon_freq_max_hz),   // 11
        params.zero_pad_factor.to_string(),         // 12
    ]).context("Failed to write CSV metadata")?;

    // Write column labels (row 2)
    writer.write_record(&[
        "time_sec",
        "frequency_hz",
        "magnitude",
        "phase_rad"
    ]).context("Failed to write CSV header")?;

    // Write data (row 3+)
    for frame in &spectrogram.frames {
        let time = frame.time_seconds;

        for i in 0..frame.frequencies.len() {
            writer.write_record(&[
                format!("{:.5}", time),
                format!("{:.4}", frame.frequencies[i]),
                format!("{:.6}", frame.magnitudes[i]),
                format!("{:.6}", frame.phases[i]),
            ]).context("Failed to write CSV record")?;
        }
    }

    writer.flush().context("Failed to flush CSV writer")?;

    Ok(())
}

/// Returns (Spectrogram, FftParams, optional recon params)
pub fn import_from_csv<P: AsRef<Path>>(path: P) -> Result<(Spectrogram, FftParams, Option<(usize, f32, f32)>)> {
    use csv::ReaderBuilder;

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)  // Allow rows with different numbers of fields
        .from_path(&path)
        .with_context(|| format!("Failed to open CSV file: {:?}", path.as_ref()))?;

    let mut records = reader.records();

    // Read metadata (row 1)
    let metadata = records.next()
        .ok_or_else(|| anyhow::anyhow!("CSV file is empty"))?
        .context("Failed to read metadata row")?;

    if metadata.len() < 9 {
        anyhow::bail!("Invalid metadata row - expected at least 9 fields, got {}", metadata.len());
    }

    let sample_rate: u32 = metadata[0].parse()
        .context("Invalid sample_rate in metadata")?;
    let window_length: usize = metadata[1].parse()
        .context("Invalid window_length in metadata")?;
    let _hop_length: usize = metadata[2].parse()
        .context("Invalid hop_length in metadata")?;
    let overlap_percent: f32 = metadata[3].parse()
        .context("Invalid overlap_percent in metadata")?;

    let window_type = if metadata[4].starts_with("Kaiser_") {
        let beta: f32 = metadata[4].trim_start_matches("Kaiser_").parse()
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

    let use_center: bool = metadata[5].parse()
        .context("Invalid use_center in metadata")?;
    let start_sample: usize = metadata[7].parse()
        .context("Invalid start_sample in metadata")?;
    let stop_sample: usize = metadata[8].parse()
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

    // Skip column labels (row 2)
    records.next();

    // Read data rows
    let mut frames_map: std::collections::BTreeMap<String, Vec<(f32, f32, f32)>> = std::collections::BTreeMap::new();

    for result in records {
        let record = result.context("Failed to read CSV record")?;

        if record.len() < 4 {
            continue;
        }

        let time_sec: String = record[0].to_string();
        let frequency_hz: f32 = record[1].parse().unwrap_or(0.0);
        let magnitude: f32 = record[2].parse().unwrap_or(0.0);
        let phase_rad: f32 = record[3].parse().unwrap_or(0.0);

        frames_map.entry(time_sec)
            .or_insert_with(Vec::new)
            .push((frequency_hz, magnitude, phase_rad));
    }

    // Build frames
    let mut frames = Vec::new();
    for (time_str, bins) in frames_map {
        let time_seconds: f64 = time_str.parse().unwrap_or(0.0);

        let mut frequencies = Vec::new();
        let mut magnitudes = Vec::new();
        let mut phases = Vec::new();

        for (freq, mag, phase) in bins {
            frequencies.push(freq);
            magnitudes.push(mag);
            phases.push(phase);
        }

        frames.push(FftFrame {
            time_seconds,
            frequencies,
            magnitudes,
            phases,
        });
    }

    let spectrogram = Spectrogram::from_frames(frames);

    let params = FftParams {
        window_length,
        overlap_percent,
        window_type,
        use_center,
        start_sample: start_sample,
        stop_sample: stop_sample,
        time_unit: TimeUnit::Seconds,
        sample_rate,
        zero_pad_factor,
    };

    Ok((spectrogram, params, recon_params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_roundtrip() {
        let frame = FftFrame {
            time_seconds: 0.0,
            frequencies: vec![0.0, 100.0, 200.0],
            magnitudes: vec![0.5, 0.8, 0.3],
            phases: vec![0.0, 1.57, 3.14],
        };

        let spec = Spectrogram::from_frames(vec![frame]);
        let params = FftParams::default();
        let view = ViewState::default();

        let temp_path = "/tmp/test_roundtrip.csv";
        export_to_csv(&spec, &params, &view, temp_path).expect("Export should succeed");

        let (imported_spec, imported_params, recon) = import_from_csv(temp_path).expect("Import should succeed");

        assert_eq!(imported_params.sample_rate, params.sample_rate);
        assert_eq!(imported_params.window_length, params.window_length);
        assert_eq!(imported_spec.num_frames(), 1);
        assert!(recon.is_some());
        let (fc, fmin, fmax) = recon.unwrap();
        assert_eq!(fc, view.recon_freq_count);
        assert!((fmin - view.recon_freq_min_hz).abs() < 1.0);
        assert!((fmax - view.recon_freq_max_hz).abs() < 1.0);

        std::fs::remove_file(temp_path).ok();
    }

    #[test]
    fn test_csv_multiple_frames() {
        let frames = vec![
            FftFrame {
                time_seconds: 0.0,
                frequencies: vec![0.0, 100.0],
                magnitudes: vec![0.1, 0.2],
                phases: vec![0.0, 0.5],
            },
            FftFrame {
                time_seconds: 0.01,
                frequencies: vec![0.0, 100.0],
                magnitudes: vec![0.3, 0.4],
                phases: vec![1.0, 1.5],
            },
        ];

        let spec = Spectrogram::from_frames(frames);
        let params = FftParams::default();
        let view = ViewState::default();

        let temp_path = "/tmp/test_multi_frames.csv";
        export_to_csv(&spec, &params, &view, temp_path).expect("Export should succeed");

        let (imported_spec, _, _) = import_from_csv(temp_path).expect("Import should succeed");
        assert_eq!(imported_spec.num_frames(), 2);

        std::fs::remove_file(temp_path).ok();
    }
}

// ============================================================================
// AUDIO.RS - Audio Device and WAV Export
// ============================================================================
//
// This module handles audio output, including:
// - Setting up the audio device for real-time playback
// - Exporting rendered audio to WAV files
//
// WAV EXPORT:
// When enabled, the engine renders the entire song to a buffer first,
// then writes it to a WAV file. This is useful for:
// - Sharing your music
// - Loading into a DAW for further processing
// - Archiving compositions
//
// AUDIO DEVICE:
// Uses miniaudio for cross-platform audio output. The audio callback
// pulls samples from the playback engine in real-time.
// ============================================================================

use std::fs::File;
use std::io::{Write, BufWriter};
use std::path::Path;

// ============================================================================
// WAV FILE FORMAT
// ============================================================================
//
// WAV is a simple uncompressed audio format. The structure is:
// 1. RIFF header (12 bytes)
// 2. Format chunk (24 bytes)
// 3. Data chunk header (8 bytes)
// 4. Audio data (variable length)
//
// We use:
// - 32-bit float samples (Format tag 3 = IEEE float)
// - 2 channels (stereo)
// - Variable sample rate (typically 48000)
// ============================================================================

/// WAV format constants
const WAV_FORMAT_PCM: u16 = 1;          // Standard PCM
const WAV_FORMAT_IEEE_FLOAT: u16 = 3;   // 32-bit float

/// Writes audio data to a WAV file
///
/// Parameters:
/// - path: The file path to write to
/// - samples: Interleaved stereo samples (L R L R ...) in -1.0 to 1.0 range
/// - sample_rate: Sample rate in Hz
/// - use_float: If true, writes 32-bit float WAV. If false, writes 16-bit PCM.
///
/// Returns: Ok(()) on success, Err with message on failure
pub fn write_wav_file(
    path: &Path,
    samples: &[f32],
    sample_rate: u32,
    use_float: bool,
) -> Result<(), String> {
    // Validate input
    if samples.is_empty() {
        return Err("No samples to write".to_string());
    }

    if samples.len() % 2 != 0 {
        return Err("Sample count must be even (stereo)".to_string());
    }

    // Create the file
    let file = File::create(path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    let mut writer = BufWriter::new(file);

    // Calculate sizes
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = if use_float { 32 } else { 16 };
    let bytes_per_sample = bits_per_sample / 8;
    let block_align = num_channels * bytes_per_sample;
    let byte_rate = sample_rate * block_align as u32;
    let format_tag = if use_float { WAV_FORMAT_IEEE_FLOAT } else { WAV_FORMAT_PCM };

    // For float format, we need the 'fact' chunk
    let has_fact_chunk = use_float;

    // Calculate audio data size
    // Total samples = samples.len() (already interleaved stereo)
    // Bytes of audio data = samples.len() * bytes_per_sample
    let audio_data_bytes = if use_float {
        samples.len() as u32 * 4 // 4 bytes per f32
    } else {
        samples.len() as u32 * 2 // 2 bytes per i16
    };

    let riff_chunk_size = 4 + // "WAVE"
        8 + 16 + // fmt chunk header + data
        (if has_fact_chunk { 8 + 4 } else { 0 }) + // fact chunk if needed
        8 + // data chunk header
        audio_data_bytes;

    // ---- Write RIFF Header ----
    writer.write_all(b"RIFF")
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&riff_chunk_size.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(b"WAVE")
        .map_err(|e| format!("Write error: {}", e))?;

    // ---- Write Format Chunk ----
    writer.write_all(b"fmt ")
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&16u32.to_le_bytes()) // Chunk size (16 for PCM)
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&format_tag.to_le_bytes()) // Format tag
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&num_channels.to_le_bytes()) // Channels
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&sample_rate.to_le_bytes()) // Sample rate
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&byte_rate.to_le_bytes()) // Byte rate
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&block_align.to_le_bytes()) // Block align
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&bits_per_sample.to_le_bytes()) // Bits per sample
        .map_err(|e| format!("Write error: {}", e))?;

    // ---- Write Fact Chunk (for float format) ----
    if has_fact_chunk {
        let sample_count = samples.len() as u32 / num_channels as u32;
        writer.write_all(b"fact")
            .map_err(|e| format!("Write error: {}", e))?;
        writer.write_all(&4u32.to_le_bytes()) // Chunk size
            .map_err(|e| format!("Write error: {}", e))?;
        writer.write_all(&sample_count.to_le_bytes()) // Sample count per channel
            .map_err(|e| format!("Write error: {}", e))?;
    }

    // ---- Write Data Chunk Header ----
    writer.write_all(b"data")
        .map_err(|e| format!("Write error: {}", e))?;
    writer.write_all(&audio_data_bytes.to_le_bytes())
        .map_err(|e| format!("Write error: {}", e))?;

    // ---- Write Audio Data ----
    if use_float {
        // Write 32-bit floats directly
        for &sample in samples {
            writer.write_all(&sample.to_le_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    } else {
        // Convert to 16-bit PCM
        for &sample in samples {
            // Clamp and scale to i16 range
            let clamped = sample.clamp(-1.0, 1.0);
            let scaled = (clamped * 32767.0) as i16;
            writer.write_all(&scaled.to_le_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
        }
    }

    // Flush and finish
    writer.flush()
        .map_err(|e| format!("Flush error: {}", e))?;

    Ok(())
}

/// Generates a default output filename based on the input filename
/// "song.csv" -> "song.wav"
pub fn generate_wav_filename(csv_path: &str) -> String {
    let path = Path::new(csv_path);

    if let Some(stem) = path.file_stem() {
        if let Some(parent) = path.parent() {
            if parent.as_os_str().is_empty() {
                format!("{}.wav", stem.to_string_lossy())
            } else {
                format!("{}/{}.wav", parent.display(), stem.to_string_lossy())
            }
        } else {
            format!("{}.wav", stem.to_string_lossy())
        }
    } else {
        "output.wav".to_string()
    }
}

// ============================================================================
// AUDIO STATISTICS
// ============================================================================

/// Statistics about rendered audio
#[derive(Clone, Debug)]
pub struct AudioStatistics {
    /// Total number of samples (per channel)
    pub sample_count: usize,

    /// Duration in seconds
    pub duration_seconds: f32,

    /// Peak amplitude (absolute value)
    pub peak_amplitude: f32,

    /// RMS (root mean square) amplitude
    pub rms_amplitude: f32,

    /// Number of samples that clipped (exceeded -1.0 or 1.0)
    pub clipped_samples: usize,
}

/// Analyzes audio buffer and returns statistics
pub fn analyze_audio(samples: &[f32], sample_rate: u32) -> AudioStatistics {
    if samples.is_empty() {
        return AudioStatistics {
            sample_count: 0,
            duration_seconds: 0.0,
            peak_amplitude: 0.0,
            rms_amplitude: 0.0,
            clipped_samples: 0,
        };
    }

    let sample_count = samples.len() / 2; // Stereo
    let duration_seconds = sample_count as f32 / sample_rate as f32;

    let mut peak_amplitude = 0.0_f32;
    let mut sum_squared = 0.0_f64;
    let mut clipped_samples = 0_usize;

    for &sample in samples {
        let abs_sample = sample.abs();

        if abs_sample > peak_amplitude {
            peak_amplitude = abs_sample;
        }

        sum_squared += (sample as f64) * (sample as f64);

        if abs_sample > 1.0 {
            clipped_samples += 1;
        }
    }

    let rms_amplitude = (sum_squared / samples.len() as f64).sqrt() as f32;

    AudioStatistics {
        sample_count,
        duration_seconds,
        peak_amplitude,
        rms_amplitude,
        clipped_samples,
    }
}

/// Normalizes audio to a target peak level
///
/// Parameters:
/// - samples: Mutable slice of audio samples
/// - target_peak: The target peak amplitude (typically 0.9 to leave headroom)
///
/// Returns: The gain factor that was applied
pub fn normalize_audio(samples: &mut [f32], target_peak: f32) -> f32 {
    if samples.is_empty() {
        return 1.0;
    }

    // Find current peak
    let current_peak = samples.iter()
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max);

    if current_peak < 0.0001 {
        // Audio is essentially silent
        return 1.0;
    }

    // Calculate and apply gain
    let gain = target_peak / current_peak;

    for sample in samples.iter_mut() {
        *sample *= gain;
    }

    gain
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_generate_wav_filename() {
        assert_eq!(generate_wav_filename("song.csv"), "song.wav");
        assert_eq!(generate_wav_filename("assets/song.csv"), "assets/song.wav");
        assert_eq!(generate_wav_filename("my_music.csv"), "my_music.wav");
    }

    #[test]
    fn test_analyze_audio() {
        // Create a simple sine wave
        let samples: Vec<f32> = (0..1000)
            .map(|i| (i as f32 * 0.1).sin() * 0.5)
            .collect();

        let stats = analyze_audio(&samples, 48000);

        assert!(stats.peak_amplitude > 0.0);
        assert!(stats.peak_amplitude <= 0.5);
        assert_eq!(stats.clipped_samples, 0);
    }

    #[test]
    fn test_normalize_audio() {
        let mut samples = vec![0.25, -0.25, 0.5, -0.5];

        let gain = normalize_audio(&mut samples, 1.0);

        assert!((gain - 2.0).abs() < 0.001);
        assert!((samples[2] - 1.0).abs() < 0.001);
    }
}

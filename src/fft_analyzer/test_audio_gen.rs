/// Generate test WAV files for FFT analyzer testing
/// 
/// Run with: cargo run --bin test_audio_gen

use hound::{WavWriter, WavSpec, SampleFormat};
use std::f32::consts::PI;

fn main() {
    println!("Generating test audio files...");

    // 1. Pure sine wave at 440 Hz (A4 note)
    generate_sine_wave("test_sine_440hz.wav", 440.0, 2.0);
    
    // 2. Chirp (sweep from 100 Hz to 1000 Hz)
    generate_chirp("test_chirp.wav", 100.0, 1000.0, 3.0);
    
    // 3. Multi-tone (combination of several frequencies)
    generate_multitone("test_multitone.wav", &[220.0, 440.0, 880.0], 2.0);
    
    // 4. White noise
    generate_white_noise("test_noise.wav", 2.0);

    println!("Test files generated successfully!");
    println!("  - test_sine_440hz.wav  (pure 440 Hz tone)");
    println!("  - test_chirp.wav       (100-1000 Hz sweep)");
    println!("  - test_multitone.wav   (220, 440, 880 Hz)");
    println!("  - test_noise.wav       (white noise)");
}

fn generate_sine_wave(filename: &str, frequency: f32, duration: f32) {
    let sample_rate = 48000;
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(filename, spec).unwrap();
    let num_samples = (sample_rate as f32 * duration) as usize;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (2.0 * PI * frequency * t).sin();
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap();
    }

    writer.finalize().unwrap();
}

fn generate_chirp(filename: &str, start_freq: f32, end_freq: f32, duration: f32) {
    let sample_rate = 48000;
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(filename, spec).unwrap();
    let num_samples = (sample_rate as f32 * duration) as usize;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let progress = t / duration;
        let freq = start_freq + (end_freq - start_freq) * progress;
        let phase = 2.0 * PI * freq * t;
        let sample = phase.sin();
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap();
    }

    writer.finalize().unwrap();
}

fn generate_multitone(filename: &str, frequencies: &[f32], duration: f32) {
    let sample_rate = 48000;
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(filename, spec).unwrap();
    let num_samples = (sample_rate as f32 * duration) as usize;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let mut sample = 0.0;
        
        for &freq in frequencies {
            sample += (2.0 * PI * freq * t).sin() / frequencies.len() as f32;
        }
        
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap();
    }

    writer.finalize().unwrap();
}

fn generate_white_noise(filename: &str, duration: f32) {
    use rand::Rng;
    
    let sample_rate = 48000;
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(filename, spec).unwrap();
    let num_samples = (sample_rate as f32 * duration) as usize;
    let mut rng = rand::rng();

    for _ in 0..num_samples {
        let sample: f32 = rng.random_range(-1.0..1.0);
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap();
    }

    writer.finalize().unwrap();
}

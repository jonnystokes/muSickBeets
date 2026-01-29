// ============================================================================
// MUSICKBEETS - CSV-DRIVEN MUSIC TRACKER SYNTHESIZER
// ============================================================================
//
// Welcome to muSickBeets! This is a real-time audio synthesizer that reads
// music from CSV files. It's designed for composers who want precise control
// over every note and effect without needing a complex DAW.
//
// SYSTEM OVERVIEW:
// ================
//
// 1. PARSER (parser.rs)
//    Reads CSV song files and converts them into playable actions.
//    Each row is a time step, each column is a channel/voice.
//    Very forgiving - handles sloppy input gracefully.
//
// 2. INSTRUMENTS (instruments.rs)
//    Sound generators: sine, trisaw, square, noise, pulse.
//    Registry pattern - easy to add new instruments.
//    Each instrument has parameters and generates audio samples.
//
// 3. EFFECTS (effects.rs)
//    Modify sounds: amplitude, pan, vibrato, tremolo, bitcrush, distortion.
//    Channel effects (per voice) and master effects (whole mix).
//    Includes reverb_1 (simple) and reverb_2 (advanced algorithmic).
//
// 4. ENVELOPES (envelope.rs)
//    Shape sound over time: ADSR (Attack, Decay, Sustain, Release).
//    Linear, exponential, and logarithmic curves.
//    Registry pattern - easy to add envelope presets.
//
// 5. CHANNELS (channel.rs)
//    Individual voices that play one note at a time.
//    Handle pitch glides, instrument crossfades, effect transitions.
//
// 6. MASTER BUS (master_bus.rs)
//    Final mixing stage - applies global effects to combined output.
//    Reverb, delay, chorus, master volume/pan.
//
// 7. ENGINE (engine.rs)
//    Coordinates everything - sequencer, mixing, playback.
//    Advances through song rows, dispatches actions to channels.
//
// 8. AUDIO (audio.rs)
//    Output handling - real-time playback and WAV export.
//
// 9. HELPER (helper.rs)
//    Utility functions - math, frequency tables, conversions.
//
// HOW TO USE:
// ===========
// 1. Edit the configuration below to your liking
// 2. Create a CSV song file (see assets/song.csv for example)
// 3. Run: cargo run --release
// 4. Listen to your creation!
//
// HOW TO ADD INSTRUMENTS:
// =======================
// 1. Open src/instruments.rs
// 2. Add a new InstrumentDefinition to INSTRUMENT_REGISTRY
// 3. Create the sample generation function
// 4. Done! The parser automatically recognizes it.
//
// HOW TO ADD EFFECTS:
// ===================
// 1. Open src/effects.rs
// 2. Add to CHANNEL_EFFECT_REGISTRY or MASTER_EFFECT_REGISTRY
// 3. Add processing logic in the appropriate apply function
//
// HOW TO ADD ENVELOPES:
// =====================
// 1. Open src/envelope.rs
// 2. Add a new EnvelopeDefinition to ENVELOPE_REGISTRY
// 3. Set the timing and curve parameters
//
// ============================================================================

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================
//
// These statements tell Rust about our module files.
// Each module is in its own file in the src/ directory.
// ============================================================================

mod helper;      // Math utilities, frequency table, shared algorithms
mod envelope;    // ADSR envelope system
mod instruments; // Sound generators (sine, square, noise, pulse, etc.)
mod effects;     // Audio effects (reverb, delay, chorus, etc.)
mod channel;     // Per-channel synthesis and state
mod master_bus;  // Master output bus and global effects
mod parser;      // CSV song file parser
mod engine;      // Playback engine and sequencer
mod audio;       // WAV export and audio utilities

// ============================================================================
// EXTERNAL DEPENDENCIES
// ============================================================================

use miniaudio::{Context, Device, DeviceConfig, DeviceType, Format, RawDevice, FramesMut, Frames};
use std::sync::{Arc, Mutex};
use std::{fs, thread, time::Duration, env, path::Path};

// Import from our modules
use crate::helper::FrequencyTable;
use crate::parser::{parse_song, DebugLevel, MissingCellBehavior};
use crate::engine::{PlaybackEngine, EngineConfig};
use crate::audio::{write_wav_file, generate_wav_filename, analyze_audio};

// ============================================================================
// CONFIGURATION
// ============================================================================
//
// Edit these values to customize the synthesizer behavior.
// All timing is in seconds, all frequencies in Hz.
// ============================================================================

// ---- File Settings ----

/// Path to the song CSV file (default, can be overridden by command line)
const SONG_FILE_PATH: &str = "assets/song.csv";

// ---- Audio Settings ----

/// Sample rate in Hz (48000 is CD quality, 44100 is also common)
/// Higher = better quality but more CPU usage
const SAMPLE_RATE: u32 = 48000;

/// Number of audio channels (voices) to use
/// Each column in the CSV is one channel
/// More channels = more polyphony but more CPU usage
const CHANNEL_COUNT: usize = 12;

/// How long each row in the CSV plays (in seconds)
/// 0.25 = 4 rows per second = 240 BPM with quarter notes
/// 0.5 = 2 rows per second = 120 BPM with quarter notes
const TICK_DURATION_SECONDS: f32 = 0.25;

/// Audio buffer size (samples per callback)
/// Smaller = lower latency but more CPU interrupts
/// 960 samples at 48kHz = 20ms latency
const AUDIO_BUFFER_SIZE: u32 = 960;

// ---- Envelope Settings ----

/// Default attack time for new notes (seconds)
/// How long it takes for a note to reach full volume
const DEFAULT_ATTACK_SECONDS: f32 = 0.10;

/// Default release time for slow release / empty cells (seconds)
/// How long it takes for a note to fade to silence
const DEFAULT_RELEASE_SECONDS: f32 = 2.0;

/// Fast release time for '.' command (seconds)
/// Quick fade to avoid pops when cutting notes short
const FAST_RELEASE_SECONDS: f32 = 0.05;

// ---- Parser Settings ----

/// What to do when a CSV row has fewer cells than CHANNEL_COUNT
/// Sustain = keep playing the current note
/// SlowRelease = fade out the current note
const MISSING_CELL_BEHAVIOR: MissingCellBehavior = MissingCellBehavior::SlowRelease;

// ---- Debug Settings ----

/// How much debug output to show
/// Off = silent (production mode)
/// Basic = parsing info, playback status, errors only
/// Verbose = + channel activity, effect changes
/// Detailed = + every token parsed, internal state (very noisy)
const DEBUG_LEVEL: DebugLevel = DebugLevel::Basic;

// ---- WAV Export Settings ----

/// Whether to export to WAV file before playing
/// If true, renders to file first, then plays the file
/// If false, plays in real-time directly
const EXPORT_TO_WAV: bool = false;

/// Whether to normalize the output when exporting to WAV
/// Normalization adjusts volume so the loudest peak hits the target level
const NORMALIZE_WAV: bool = true;

/// Target peak level for normalization (0.0 to 1.0)
/// 0.9 leaves a bit of headroom, 1.0 uses full range
const NORMALIZE_TARGET_PEAK: f32 = 0.9;

// ---- Validate-Only Mode ----

/// If true, just parse the song and report errors, don't play
/// Useful for checking your CSV file for mistakes
const VALIDATE_ONLY: bool = false;

// ============================================================================
// MAIN FUNCTION
// ============================================================================

fn main() {
    // Print welcome banner
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║   muSickBeets - CSV-Driven Music Tracker Synthesizer      ║");
    println!("║   Version 2.0 - Modular Architecture                      ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    // ---- Parse Command Line Arguments ----
    // Usage: tracker [song_file.csv]
    let args: Vec<String> = env::args().collect();
    let song_path = if args.len() > 1 {
        &args[1]
    } else {
        SONG_FILE_PATH
    };

    println!("[MAIN] Song file: {}", song_path);
    println!("[MAIN] Sample rate: {} Hz", SAMPLE_RATE);
    println!("[MAIN] Channels: {}", CHANNEL_COUNT);
    println!("[MAIN] Tick duration: {:.3}s", TICK_DURATION_SECONDS);
    println!("[MAIN] Debug level: {:?}", DEBUG_LEVEL);

    // ---- Load Song File ----
    let song_text = match fs::read_to_string(song_path) {
        Ok(text) => {
            println!("[MAIN] Loaded song file ({} bytes)", text.len());
            text
        }
        Err(error) => {
            eprintln!("[ERROR] Failed to read song file '{}': {}", song_path, error);
            eprintln!("[HINT] Make sure the file exists and is readable.");
            eprintln!("[HINT] Usage: tracker [song_file.csv]");
            return;
        }
    };

    // ---- Initialize Frequency Table ----
    // Pre-compute all note frequencies for fast lookup during playback
    println!("[MAIN] Building frequency table (octaves 0-20)...");
    let frequency_table = FrequencyTable::new();

    // ---- Parse Song ----
    println!("[MAIN] Parsing song...");
    let song_data = parse_song(
        &song_text,
        &frequency_table,
        CHANNEL_COUNT,
        MISSING_CELL_BEHAVIOR,
        DEBUG_LEVEL,
    );

    // Report parsing results
    println!(
        "[MAIN] Parsed {} rows, {} errors",
        song_data.row_count(),
        song_data.errors.len()
    );

    // Print any errors
    if !song_data.errors.is_empty() {
        println!("\n[PARSER MESSAGES]");
        song_data.print_errors();
        println!();
    }

    // Check for fatal errors
    if song_data.has_fatal_errors() {
        eprintln!("[ERROR] Fatal parsing errors encountered. Cannot play.");
        return;
    }

    // Validate-only mode
    if VALIDATE_ONLY {
        println!("[MAIN] Validate-only mode - parsing complete.");
        if song_data.errors.is_empty() {
            println!("[MAIN] No errors found! Song is valid.");
        } else {
            println!("[MAIN] Found {} warnings/errors.", song_data.errors.len());
        }
        return;
    }

    // Check for empty song
    if song_data.row_count() == 0 {
        eprintln!("[ERROR] Song has no rows to play!");
        return;
    }

    // ---- Create Engine Configuration ----
    let engine_config = EngineConfig {
        sample_rate: SAMPLE_RATE,
        channel_count: CHANNEL_COUNT,
        tick_duration_seconds: TICK_DURATION_SECONDS,
        default_attack_seconds: DEFAULT_ATTACK_SECONDS,
        default_release_seconds: DEFAULT_RELEASE_SECONDS,
        fast_release_seconds: FAST_RELEASE_SECONDS,
        debug_level: DEBUG_LEVEL,
    };

    // Calculate duration
    let total_duration_seconds = song_data.row_count() as f32 * TICK_DURATION_SECONDS;
    println!(
        "[MAIN] Song duration: {:.2}s ({} rows)",
        total_duration_seconds,
        song_data.row_count()
    );

    // ---- WAV Export Mode ----
    if EXPORT_TO_WAV {
        export_to_wav(song_data, engine_config, song_path);
        return;
    }

    // ---- Real-Time Playback Mode ----
    play_realtime(song_data, engine_config, total_duration_seconds);
}

/// Exports the song to a WAV file
fn export_to_wav(
    song_data: crate::parser::SongData,
    engine_config: EngineConfig,
    song_path: &str,
) {
    println!("\n[EXPORT] Rendering to WAV...");

    // Create engine and render
    let mut engine = PlaybackEngine::new(song_data, engine_config.clone());
    let mut samples = engine.render_to_buffer();

    // Analyze
    let stats = analyze_audio(&samples, engine_config.sample_rate);
    println!("[EXPORT] Rendered {} samples ({:.2}s)", stats.sample_count, stats.duration_seconds);
    println!("[EXPORT] Peak amplitude: {:.3}", stats.peak_amplitude);
    println!("[EXPORT] RMS amplitude: {:.3}", stats.rms_amplitude);

    if stats.clipped_samples > 0 {
        println!("[WARNING] {} samples clipped!", stats.clipped_samples);
    }

    // Normalize if requested
    if NORMALIZE_WAV {
        let gain = crate::audio::normalize_audio(&mut samples, NORMALIZE_TARGET_PEAK);
        println!("[EXPORT] Normalized with gain: {:.3}", gain);
    }

    // Generate output filename
    let wav_path = generate_wav_filename(song_path);
    println!("[EXPORT] Writing to: {}", wav_path);

    // Write WAV file
    match write_wav_file(Path::new(&wav_path), &samples, engine_config.sample_rate, false) {
        Ok(()) => {
            println!("[EXPORT] Successfully wrote WAV file!");
        }
        Err(error) => {
            eprintln!("[ERROR] Failed to write WAV: {}", error);
        }
    }
}

/// Plays the song in real-time
fn play_realtime(
    song_data: crate::parser::SongData,
    engine_config: EngineConfig,
    total_duration_seconds: f32,
) {
    // Create the playback engine wrapped in Arc<Mutex> for thread safety
    let engine = Arc::new(Mutex::new(PlaybackEngine::new(song_data, engine_config)));
    let engine_for_callback = Arc::clone(&engine);

    // ---- Initialize Audio Device ----
    println!("\n[AUDIO] Initializing miniaudio...");

    let audio_context = match Context::new(&[], None) {
        Ok(ctx) => ctx,
        Err(error) => {
            eprintln!("[ERROR] Failed to create audio context: {:?}", error);
            return;
        }
    };

    // Configure audio device
    let mut device_config = DeviceConfig::new(DeviceType::Playback);
    device_config.playback_mut().set_format(Format::F32);
    device_config.playback_mut().set_channels(2);
    device_config.set_sample_rate(SAMPLE_RATE);
    device_config.set_period_size_in_frames(AUDIO_BUFFER_SIZE);

    // Set up the audio callback
    // This function is called by the audio driver when it needs more samples
    device_config.set_data_callback(move |_device: &RawDevice, output_buffer: &mut FramesMut, _input_buffer: &Frames| {
        // Get the output buffer as f32 samples
        let samples = output_buffer.as_samples_mut::<f32>();

        // Lock the engine and process
        if let Ok(mut engine_guard) = engine_for_callback.lock() {
            engine_guard.process_frame(samples);
        }
    });

    // Create the audio device
    let audio_device: Device = match Device::new(Some(audio_context), &device_config) {
        Ok(device) => device,
        Err(error) => {
            eprintln!("[ERROR] Failed to create audio device: {:?}", error);
            return;
        }
    };

    // ---- Start Playback ----
    println!("[AUDIO] Starting playback...");

    if let Err(error) = audio_device.start() {
        eprintln!("[ERROR] Failed to start audio device: {:?}", error);
        return;
    }

    println!("\n▶ PLAYING... (duration: {:.2}s)\n", total_duration_seconds);

    // Wait for playback to finish
    // Add extra time for release tails
    let wait_time = total_duration_seconds + 2.0;
    thread::sleep(Duration::from_secs_f32(wait_time));

    // ---- Cleanup ----
    println!("\n[MAIN] Playback finished!");
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║                THANK YOU FOR LISTENING!                   ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

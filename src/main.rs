// ============================================
// RUST TRACKER SYNTH - FIXED VERSION
// ============================================
// A CSV-driven music tracker with real-time synthesis and effects.
//
// ARCHITECTURE:
// - 8+ channels of synthesis (columns in CSV, configurable)
// - Each channel can play one instrument at a time
// - Master bus processes the mixed output
// - Smooth transitions for all parameters
// - VERY FORGIVING PARSER - handles sloppy input gracefully
//
// KEY FEATURES:
// - Transition system (tr:X) for smooth parameter changes and pitch glides
// - Per-channel effects: vibrato, tremolo, bitcrush, distortion
// - Master effects: reverb, delay, amplitude, pan
// - Instrument crossfading during transitions
// - MOD-tracker style envelope control
// - Instruments can be pitch-based (sine, square) or pitchless (noise)
//
// USAGE:
// 1. Create a CSV file with one row per time step
// 2. Each column represents a channel (0-N)
// 3. Use transition (tr:X) to smoothly change pitch, instrument, or effects
//
// CELL SYNTAX:
// - "c4 sine a:0.8"           → Play C4 with sine wave at 80% volume
// - "c4 sine tr:2 a:0.5"      → Glide pitch to C4 and fade to 50% over 2 seconds
// - "-"                       → Sustain (keep playing current sound)
// - "- a:0.5"                 → Sustain with effect change
// - "."                       → Fast release (fade out in 50ms, avoids pops)
// - ""  (empty)               → Slow release (fade out based on MISSING_CELL_BEHAVIOR)
// - "master reverb:0.5'0.3"   → Master reverb (room=0.5, mix=0.3)
// - "# comment"               → Comments (# or // to end of line)
// - "noise a:0.5"             → Play noise (no pitch required)
//
// TRANSITION BEHAVIOR:
// When tr:X is used:
// - If channel is already playing → smooth glide (no retrigger)
// - If channel is silent → normal trigger with transition
// - Effects always transition smoothly over X seconds

use miniaudio::{Context, Device, DeviceConfig, DeviceType, Format};
use std::f32::consts::PI;
use std::{thread, time::Duration, fs};
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

// ============================================
// CONFIGURATION
// ============================================

const USE_FILE: bool = false;
const SONG_FILE_PATH: &str = "assets/song.csv";

const NUM_CHANNELS: usize = 12;  // Number of sound channels (adjust based on CSV)
const SAMPLE_RATE: u32 = 48000;
const TICK_DURATION_SEC: f32 = 0.25;   // How long each row plays

const DEFAULT_ATTACK_SEC: f32 = 0.10;
const DEFAULT_RELEASE_SEC: f32 = 2.0;      // Slow release for empty cells (2 seconds)
const FAST_RELEASE_SEC: f32 = 0.05;        // Fast release for '.' to avoid pops (50ms)

// NEW: Control what happens when a row has fewer cells than NUM_CHANNELS
#[derive(Clone, Copy, Debug, PartialEq)]
enum MissingCellBehavior {
    Sustain,      // Keep playing (like "-")
    SlowRelease,  // Fade out slowly (like "")
}
const MISSING_CELL_BEHAVIOR: MissingCellBehavior = MissingCellBehavior::SlowRelease;

const DEBUG_PARSER: bool = true;
const DEBUG_CHANNELS: bool = false;
const DEBUG_MASTER: bool = true;
const DEBUG_EFFECTS: bool = true;
const DEBUG_TIMING: bool = false;
const DEBUG_PLAYBACK: bool = true;

// Test song with all features
const SONG_STRING: &str = r#"
Voice0,Voice1,Voice2,Voice3,Voice4,Voice5,Voice6,Voice7,Voice8
,c4 sine a:0.8 p:-0.5,e4 SINE a:0.8 p:0.5,g4 TriSaw:0.5 a:0.6,-,-,-,-,-
rv:0.2'0.3,-,-,-,c5 noise a:0.4,-,-,-,-
-,c4 v:4'0.3,e4 v:4'0.3,g4 tr:1.0 a:0.3,-,c5 square a:0.7,-,-,-
-,d4 s:0.5,f4 s:0.5,a4,-,-,-,-,-
dl:0.25'0.4,.,.,c5 TriSaw:-0.8,c5 noise a:0.4,c4 sine b:4,e4 sine t:3'0.5,-,-
-,-,-,-,-,-,g4 a:0.7,-,-
cl,c4 sine a:0.9,e4 a:0.9,g4 a:0.9,.,-,.,c5 TriSaw:0.8 a:0.6,-
-,c4 d:0.3,e4 d:0.3,g4 d:0.3,-,c4 a:0.8,-,-,a4 sine a:0.5
-,-,-,-,-,-,-,-,a4 v:2'0.4 tr:0.5
-,.,.,b:6,-,.,.,.,a4 cl:0.3 a:0.2
-,-,-,.,-,-,-,-,.
,c3 sine a:1.0,e3 a:1.0,g3 a:1.0,-,-,-,-,-
-,-,-,-,-,-,-,-,-
.,.,.,.,-,-,-,-,-
"#;

// ============================================
// DATA STRUCTURES
// ============================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum EnvelopeState {
    Idle,
    Attack,
    Sustain,
    Release,
}

// ============================================
// EFFECT STATE
// ============================================
#[derive(Clone, Debug)]
struct EffectState {
    amplitude: f32,
    pan: f32,
    vibrato_rate_hz: f32,
    vibrato_depth_semitones: f32,
    vibrato_phase: f32,
    tremolo_rate_hz: f32,
    tremolo_depth: f32,
    tremolo_phase: f32,
    bitcrush_bits: u8,
    distortion_amount: f32,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            amplitude: 1.0,
            pan: 0.0,
            vibrato_rate_hz: 0.0,
            vibrato_depth_semitones: 0.0,
            vibrato_phase: 0.0,
            tremolo_rate_hz: 0.0,
            tremolo_depth: 0.0,
            tremolo_phase: 0.0,
            bitcrush_bits: 16,
            distortion_amount: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Transition {
    duration_samples: u32,
    elapsed_samples: u32,
    start_state: EffectState,
}

// ============================================
// CHANNEL
// ============================================
#[derive(Clone, Debug)]
struct Channel {
    id: usize,
    active: bool,
    frequency_hz: f32,
    phase: f32,
    instrument_id: usize,
    instrument_params: Vec<f32>,
    envelope_state: EnvelopeState,
    envelope_level: f32,
    attack_time_sec: f32,
    release_time_sec: f32,
    release_start_level: f32,
    release_start_time: u64,
    current_effects: EffectState,
    target_effects: Option<EffectState>,
    transition: Option<Transition>,
    slide_active: bool,
    slide_start_freq: f32,
    slide_target_freq: f32,
    slide_duration_sec: f32,
    slide_elapsed_sec: f32,
    crossfade_active: bool,
    crossfade_from_inst: usize,
    crossfade_to_inst: usize,
    time_samples: u64,
    rng_state: u32,
}

impl Channel {
    fn new(id: usize) -> Self {
        Self {
            id,
            active: false,
            frequency_hz: 440.0,
            phase: 0.0,
            instrument_id: 1,
            instrument_params: vec![],
            envelope_state: EnvelopeState::Idle,
            envelope_level: 0.0,
            attack_time_sec: DEFAULT_ATTACK_SEC,
            release_time_sec: DEFAULT_RELEASE_SEC,
            release_start_level: 0.0,
            release_start_time: 0,
            current_effects: EffectState::default(),
            target_effects: None,
            transition: None,
            slide_active: false,
            slide_start_freq: 440.0,
            slide_target_freq: 440.0,
            slide_duration_sec: 0.0,
            slide_elapsed_sec: 0.0,
            crossfade_active: false,
            crossfade_from_inst: 1,
            crossfade_to_inst: 1,
            time_samples: 0,
            rng_state: ((id as u32).wrapping_mul(1103515245).wrapping_add(12345)),
        }
    }
}

// ============================================
// MASTER BUS
// ============================================
#[derive(Clone, Debug)]
struct MasterBus {
    amplitude: f32,
    pan: f32,
    reverb_enabled: bool,
    reverb_room_size: f32,
    reverb_mix: f32,
    reverb_buffer: Vec<f32>,
    reverb_pos: usize,
    delay_enabled: bool,
    delay_time_samples: u32,
    delay_feedback: f32,
    delay_buffer_l: Vec<f32>,
    delay_buffer_r: Vec<f32>,
    delay_write_pos: usize,
    transition_active: bool,
    transition_duration_samples: u32,
    transition_elapsed_samples: u32,
    start_amplitude: f32,
    start_pan: f32,
    start_reverb_room_size: f32,
    start_reverb_mix: f32,
    start_delay_time_samples: u32,
    start_delay_feedback: f32,
    start_reverb_enabled: bool,
    start_delay_enabled: bool,
    target_amplitude: f32,
    target_pan: f32,
    target_reverb_room_size: f32,
    target_reverb_mix: f32,
    target_delay_time_samples: u32,
    target_delay_feedback: f32,
    target_reverb_enabled: bool,
    target_delay_enabled: bool,
}

impl MasterBus {
    fn new() -> Self {
        let max_delay_samples = (SAMPLE_RATE as f32 * 2.0) as usize;
        Self {
            amplitude: 1.0,
            pan: 0.0,
            reverb_enabled: false,
            reverb_room_size: 0.5,
            reverb_mix: 0.3,
            reverb_buffer: vec![0.0; max_delay_samples],
            reverb_pos: 0,
            delay_enabled: false,
            delay_time_samples: SAMPLE_RATE / 4,
            delay_feedback: 0.3,
            delay_buffer_l: vec![0.0; max_delay_samples],
            delay_buffer_r: vec![0.0; max_delay_samples],
            delay_write_pos: 0,
            transition_active: false,
            transition_duration_samples: 0,
            transition_elapsed_samples: 0,
            start_amplitude: 1.0,
            start_pan: 0.0,
            start_reverb_room_size: 0.5,
            start_reverb_mix: 0.3,
            start_delay_time_samples: SAMPLE_RATE / 4,
            start_delay_feedback: 0.3,
            start_reverb_enabled: false,
            start_delay_enabled: false,
            target_amplitude: 1.0,
            target_pan: 0.0,
            target_reverb_room_size: 0.5,
            target_reverb_mix: 0.3,
            target_delay_time_samples: SAMPLE_RATE / 4,
            target_delay_feedback: 0.3,
            target_reverb_enabled: false,
            target_delay_enabled: false,
        }
    }
}

// ============================================
// CELL ACTIONS
// ============================================
#[derive(Clone, Debug)]
enum CellAction {
    TriggerNote {
        pitch: String,
        instrument_id: usize,
        instrument_params: Vec<f32>,
        effects: EffectState,
        transition_sec: f32,
        clear_effects: bool,
    },
    TriggerPitchless {  // NEW: For instruments that don't need pitch (noise)
        instrument_id: usize,
        instrument_params: Vec<f32>,
        effects: EffectState,
        transition_sec: f32,
        clear_effects: bool,
    },
    Sustain,
    SustainWithEffects {  // NEW: "- a:0.5" sustains but changes effects
        effects: EffectState,
        transition_sec: f32,
        clear_first: bool,
    },
    FastRelease,
    SlowRelease,
    ChangeEffects {
        effects: EffectState,
        transition_sec: f32,
        clear_first: bool,
    },
    MasterEffects {
        clear_first: bool,
        transition_sec: f32,
        effects: Vec<(String, Vec<f32>)>,
    },
}

// ============================================
// SONG DATA
// ============================================
struct SongData {
    rows: Vec<Vec<CellAction>>,
    raw_lines: Vec<String>,
}

// ============================================
// PLAYBACK ENGINE
// ============================================
struct PlaybackEngine {
    song: SongData,
    current_row: usize,
    samples_in_current_row: u32,
    samples_per_row: u32,
    channels: Vec<Channel>,
    master_bus: MasterBus,
}

// ============================================
// INSTRUMENT DEFINITIONS
// ============================================
#[derive(Clone, Copy, Debug)]
enum InstrumentType {
    Master,
    Sine,
    TriSaw,
    Square,
    Noise,
}

struct InstrumentDef {
    id: usize,
    name: &'static str,
    short_name: &'static str,
    inst_type: InstrumentType,
    attack_sec: f32,
    release_sec: f32,
    requires_pitch: bool,  // NEW: Does this instrument need a note?
}

const INSTRUMENTS: &[InstrumentDef] = &[
    InstrumentDef {
        id: 0,
        name: "master",
        short_name: "master",
        inst_type: InstrumentType::Master,
        attack_sec: 0.0,
        release_sec: 0.0,
        requires_pitch: false,
    },
    InstrumentDef {
        id: 1,
        name: "sine",
        short_name: "sine",
        inst_type: InstrumentType::Sine,
        attack_sec: 0.01,
        release_sec: 0.5,
        requires_pitch: true,
    },
    InstrumentDef {
        id: 2,
        name: "trisaw",
        short_name: "trisaw",
        inst_type: InstrumentType::TriSaw,
        attack_sec: 0.01,
        release_sec: 0.3,
        requires_pitch: true,
    },
    InstrumentDef {
        id: 3,
        name: "square",
        short_name: "square",
        inst_type: InstrumentType::Square,
        attack_sec: 0.005,
        release_sec: 0.2,
        requires_pitch: true,
    },
    InstrumentDef {
        id: 4,
        name: "noise",
        short_name: "noise",
        inst_type: InstrumentType::Noise,
        attack_sec: 0.001,
        release_sec: 0.1,
        requires_pitch: false,  // NEW: Noise doesn't need pitch!
    },
];

fn find_instrument(name: &str) -> Option<usize> {
    let name_lower = name.to_lowercase();
    INSTRUMENTS.iter()
        .find(|inst| inst.name == name_lower || inst.short_name == name_lower)
        .map(|inst| inst.id)
}

// ============================================
// AUDIO GENERATION
// ============================================
fn generate_sample(
    inst_type: InstrumentType,
    phase: f32,
    params: &[f32],
    rng_state: &mut u32,
) -> f32 {
    match inst_type {
        InstrumentType::Master => 0.0,

        InstrumentType::Sine => {
            phase.sin()
        }

        InstrumentType::TriSaw => {
            let shape = if params.is_empty() { 0.0 } else { params[0].clamp(-1.0, 1.0) };
            let t = phase / (2.0 * PI);
            let peak_pos = (shape + 1.0) / 2.0;

            if t < peak_pos {
                if peak_pos > 0.0 {
                    2.0 * (t / peak_pos) - 1.0
                } else {
                    -1.0
                }
            } else {
                let remaining = 1.0 - peak_pos;
                if remaining > 0.0 {
                    1.0 - 2.0 * ((t - peak_pos) / remaining)
                } else {
                    1.0
                }
            }
        }

        InstrumentType::Square => {
            if phase.sin() > 0.0 { 1.0 } else { -1.0 }
        }

        InstrumentType::Noise => {
            *rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let normalized = (*rng_state as f32) / (u32::MAX as f32);
            normalized * 2.0 - 1.0
        }
    }
}

// ============================================
// PITCH CONVERSION
// ============================================
fn pitch_to_frequency(pitch: &str) -> f32 {
    let pitch_lower = pitch.to_lowercase();
    let chars: Vec<char> = pitch_lower.chars().collect();

    if chars.is_empty() {
        println!("[ERROR] Empty pitch string");
        return 440.0;
    }

    let note_char = chars[0];
    let mut idx = 1;

    let mut semitone_offset = 0;
    if idx < chars.len() && (chars[idx] == '#' || chars[idx] == 'b') {
        if chars[idx] == '#' {
            semitone_offset = 1;
        } else {
            semitone_offset = -1;
        }
        idx += 1;
    }

    let octave_str: String = chars[idx..].iter().collect();
    let octave: i32 = octave_str.parse().unwrap_or(4);

    let base_semitone = match note_char {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => {
            println!("[ERROR] Unknown note: {}", note_char);
            9
        }
    };

    let note_semitone = base_semitone + semitone_offset;
    let semitones_from_c4 = note_semitone + (octave - 4) * 12;
    let semitones_from_a4 = semitones_from_c4 - 9;

    let frequency = 440.0 * 2.0_f32.powf(semitones_from_a4 as f32 / 12.0);

    frequency
}

// ============================================
// PARSER - MUCH MORE FORGIVING
// ============================================

fn strip_comments(line: &str) -> &str {
    // Handle // comments (always means comment)
    if let Some(slash_pos) = line.find("//") {
        return &line[..slash_pos];
    }
    
    // Handle # comments (but preserve c#4, d#5, etc)
    if let Some(hash_pos) = line.find('#') {
        let is_sharp_note = if hash_pos > 0 {
            let before = line.as_bytes()[hash_pos - 1] as char;
            matches!(before.to_ascii_lowercase(), 'a'..='g')
        } else {
            false
        };
        
        if !is_sharp_note {
            return &line[..hash_pos];
        }
    }
    
    line
}

fn parse_song(song_str: &str) -> SongData {
    println!("\n[PARSER] ========== PARSING SONG ==========");

    let mut rows = Vec::new();
    let mut raw_lines = Vec::new();
    let mut first_row = true;
    let mut actual_row_count = 0;

    for (line_idx, line) in song_str.lines().enumerate() {
        // Strip comments FIRST
        let line = strip_comments(line);
        
        // Trim and check if empty
        let line_trimmed = line.trim();
        
        // SKIP COMPLETELY EMPTY LINES (including comment-only lines)
        if line_trimmed.is_empty() {
            if DEBUG_PARSER {
                println!("[PARSER] Line {}: Skipping empty/comment-only line", line_idx);
            }
            continue;
        }
        
        // Skip header row
        if first_row {
            first_row = false;
            if DEBUG_PARSER {
                println!("[PARSER] Line {}: Skipping header row: '{}'", line_idx, line_trimmed);
            }
            continue;
        }

        // Store raw line for debug output
        raw_lines.push(line_trimmed.to_string());
        actual_row_count += 1;

        if DEBUG_PARSER {
            println!("[PARSER] Row {}: '{}'", actual_row_count - 1, line_trimmed);
        }

        // Parse cells
        let cells: Vec<&str> = line_trimmed.split(',').collect();
        let mut row_actions = Vec::new();

        // Process up to NUM_CHANNELS
        for channel_idx in 0..NUM_CHANNELS {
            let cell = if channel_idx < cells.len() {
                cells[channel_idx].trim()
            } else {
                // MISSING CELL - use configured behavior
                if DEBUG_PARSER {
                    println!("[PARSER]   Channel {}: MISSING (using {:?})", 
                             channel_idx, MISSING_CELL_BEHAVIOR);
                }
                match MISSING_CELL_BEHAVIOR {
                    MissingCellBehavior::Sustain => {
                        row_actions.push(CellAction::Sustain);
                        continue;
                    }
                    MissingCellBehavior::SlowRelease => {
                        row_actions.push(CellAction::SlowRelease);
                        continue;
                    }
                }
            };

            if DEBUG_PARSER {
                println!("[PARSER]   Channel {}: '{}'", channel_idx, cell);
            }

            let action = parse_cell(cell, channel_idx);
            row_actions.push(action);
        }

        // Warn about extra cells
        if cells.len() > NUM_CHANNELS {
            println!("[PARSER WARNING] Row {} has {} cells but only {} channels configured. Extra cells ignored.",
                     actual_row_count - 1, cells.len(), NUM_CHANNELS);
        }

        rows.push(row_actions);
    }

    println!("[PARSER] ========== PARSING COMPLETE: {} rows ==========\n", rows.len());

    SongData { rows, raw_lines }
}

fn parse_cell(cell: &str, channel_idx: usize) -> CellAction {
    let cell = cell.trim();

    // Empty = Slow Release
    if cell.is_empty() {
        return CellAction::SlowRelease;
    }

    // Sustain (plain hyphen)
    if cell == "-" {
        return CellAction::Sustain;
    }

    // Fast Release
    if cell == "." {
        return CellAction::FastRelease;
    }

    // Split into tokens (handles multiple spaces gracefully)
    let tokens: Vec<&str> = cell.split_whitespace().collect();
    if tokens.is_empty() {
        return CellAction::SlowRelease;
    }

    // Check if first token is "-" (sustain with effects)
    if tokens[0] == "-" && tokens.len() > 1 {
        // Sustain with effects: "- a:0.5 tr:2"
        return parse_sustain_with_effects(&tokens[1..]);
    }

    // Check if first token is a note (starts with a-g)
    let first_token = tokens[0];
    let first_char = first_token.chars().next().unwrap().to_lowercase().next().unwrap();
    let is_note = matches!(first_char, 'a'..='g');

    if is_note {
        // Note trigger: "c4 sine a:0.8"
        parse_note_trigger(tokens, channel_idx)
    } else {
        // Check if first token is instrument name/number
        if let Some(inst_id) = find_instrument(first_token) {
            if inst_id == 0 {
                // Instrument 0 = master
                parse_master_effect(tokens)
            } else {
                // Non-master instrument without note
                let inst = &INSTRUMENTS[inst_id];
                if !inst.requires_pitch {
                    // Pitchless instrument (like noise) - OK!
                    parse_pitchless_trigger(tokens, channel_idx)
                } else {
                    // Requires pitch but none given - ERROR
                    println!("[PARSER ERROR] Channel {}: Instrument '{}' requires a note. Cell: '{}'", 
                             channel_idx, inst.name, cell);
                    return CellAction::SlowRelease;
                }
            }
        } else {
            // Not instrument, check if master effect or channel effect
            if is_master_effect(first_token) {
                parse_master_effect(tokens)
            } else {
                // Channel effect change: "a:0.8", "p:-0.5"
                parse_effect_change(tokens, channel_idx)
            }
        }
    }
}

// NEW: Parse sustain with effects "- a:0.5 tr:2"
fn parse_sustain_with_effects(tokens: &[&str]) -> CellAction {
    let mut effects = EffectState::default();
    let mut transition_sec = 0.0;
    let mut clear_first = false;

    // Check for clear flag
    for token in tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_first = true;
            break;
        }
    }

    // Process effects
    for token in tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }
        
        if let Some((effect_name, effect_value)) = parse_effect_token(token) {
            apply_effect_to_state(&mut effects, &effect_name, &effect_value, &mut transition_sec, &mut clear_first);
        }
    }

    CellAction::SustainWithEffects {
        effects,
        transition_sec,
        clear_first,
    }
}

// NEW: Parse pitchless instrument trigger "noise a:0.5"
fn parse_pitchless_trigger(tokens: Vec<&str>, channel_idx: usize) -> CellAction {
    let instrument_name = tokens[0];
    let instrument_id = find_instrument(instrument_name).unwrap();
    
    let instrument_params = Vec::new();
    let mut effects = EffectState::default();
    let mut transition_sec = 0.0;
    let mut clear_effects = false;

    // Check for clear flag
    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_effects = true;
            break;
        }
    }

    // Process tokens
    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();
        
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }
        
        // Check for unrecognized standalone tokens (like stray periods)
        if !token.contains(':') && token_lower != "clear" && token_lower != "cl" {
            println!("[PARSER WARNING] Channel {}: Unrecognized token '{}' - ignoring", channel_idx, token);
            continue;
        }
        
        if let Some((effect_name, effect_value)) = parse_effect_token(token) {
            apply_effect_to_state(&mut effects, &effect_name, &effect_value, &mut transition_sec, &mut clear_effects);
        }
    }

    CellAction::TriggerPitchless {
        instrument_id,
        instrument_params,
        effects,
        transition_sec,
        clear_effects,
    }
}

fn is_master_effect(token: &str) -> bool {
    if let Some(colon_pos) = token.find(':') {
        let effect_name = &token[..colon_pos].to_lowercase();
        matches!(effect_name.as_str(), "rv" | "reverb" | "dl" | "delay" | "cl" | "clear")
    } else {
        false
    }
}

fn parse_note_trigger(tokens: Vec<&str>, channel_idx: usize) -> CellAction {
    let pitch = tokens[0].to_string();
    let mut instrument_id = 1;
    let mut instrument_params = Vec::new();
    let mut effects = EffectState::default();
    let mut transition_sec = 0.0;
    let mut clear_effects = false;
    let mut seen_effects: HashSet<String> = HashSet::new();

    // FIRST PASS: Check for clear
    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_effects = true;
            break;
        }
    }

    // SECOND PASS: Process tokens
    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();
        
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }

        // Check for unrecognized standalone tokens
        if !token.contains(':') {
            // Check if it's an instrument name
            if find_instrument(token).is_none() {
                println!("[PARSER WARNING] Channel {}: Unrecognized token '{}' - ignoring", channel_idx, token);
            }
            continue;
        }

        // Check for instrument specification
        if let Some(colon_pos) = token.find(':') {
            let prefix = &token[..colon_pos].to_lowercase();
            
            if prefix == "i" || prefix == "inst" || prefix == "instrument" {
                let id_str = &token[colon_pos + 1..];
                if let Ok(id) = id_str.parse::<usize>() {
                    if id == 0 {
                        println!("[PARSER ERROR] Channel {}: Cannot play notes on instrument 0 (master)", channel_idx);
                        return CellAction::SlowRelease;
                    }
                    if id < INSTRUMENTS.len() {
                        instrument_id = id;
                        continue;
                    }
                }
            }
            
            // Check if it's instrument with params
            let inst_name = &token[..colon_pos];
            if let Some(inst_id) = find_instrument(inst_name) {
                if inst_id == 0 {
                    println!("[PARSER ERROR] Channel {}: Cannot play notes on instrument 0 (master)", channel_idx);
                    return CellAction::SlowRelease;
                }
                instrument_id = inst_id;
                let params_str = &token[colon_pos + 1..];
                instrument_params = parse_params(params_str);
                continue;
            }
            
            // Otherwise it's an effect
            if let Some((effect_name, effect_value)) = parse_effect_token(token) {
                // Check for duplicate effects
                if seen_effects.contains(&effect_name) {
                    println!("[PARSER ERROR] Channel {}: Effect '{}' specified multiple times - using first occurrence only", 
                             channel_idx, effect_name);
                    continue;
                }
                seen_effects.insert(effect_name.clone());
                
                apply_effect_to_state(&mut effects, &effect_name, &effect_value, &mut transition_sec, &mut clear_effects);
            }
        } else {
            // No colon - check if instrument
            if let Some(inst_id) = find_instrument(token) {
                if inst_id == 0 {
                    println!("[PARSER ERROR] Channel {}: Cannot play notes on instrument 0 (master)", channel_idx);
                    return CellAction::SlowRelease;
                }
                instrument_id = inst_id;
            }
        }
    }

    CellAction::TriggerNote {
        pitch,
        instrument_id,
        instrument_params,
        effects,
        transition_sec,
        clear_effects,
    }
}

fn parse_effect_change(tokens: Vec<&str>, channel_idx: usize) -> CellAction {
    let mut effects = EffectState::default();
    let mut transition_sec = 0.0;
    let mut clear_first = false;
    let mut seen_effects: HashSet<String> = HashSet::new();

    // Check for clear
    for token in &tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_first = true;
            break;
        }
    }

    // Process effects
    for token in tokens {
        let token_lower = token.to_lowercase();
        
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }
        
        if let Some((effect_name, effect_value)) = parse_effect_token(token) {
            // Check for duplicates
            if seen_effects.contains(&effect_name) {
                println!("[PARSER ERROR] Channel {}: Effect '{}' specified multiple times - using first occurrence only", 
                         channel_idx, effect_name);
                continue;
            }
            seen_effects.insert(effect_name.clone());
            
            apply_effect_to_state(&mut effects, &effect_name, &effect_value, &mut transition_sec, &mut clear_first);
        }
    }

    CellAction::ChangeEffects {
        effects,
        transition_sec,
        clear_first,
    }
}

fn parse_master_effect(tokens: Vec<&str>) -> CellAction {
    let start_idx = if tokens.is_empty() { 0 } else if find_instrument(tokens[0]).is_some() { 1 } else { 0 };
    let effect_tokens = &tokens[start_idx..];
    
    let mut should_clear = false;
    for token in effect_tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" || token_lower.starts_with("clear:") || token_lower.starts_with("cl:") {
            should_clear = true;
            break;
        }
    }
    
    let mut master_effects = Vec::new();
    let mut transition_sec = 0.0;
    let mut seen_effects: HashSet<String> = HashSet::new();
    
    for token in effect_tokens {
        let token_lower = token.to_lowercase();
        
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }
        if token_lower.starts_with("clear:") || token_lower.starts_with("cl:") {
            if let Some((_, value)) = parse_effect_token(token) {
                let params = parse_params(&value);
                if !params.is_empty() {
                    transition_sec = params[0].max(0.0);
                }
            }
            continue;
        }
        
        if let Some((effect_name, effect_value)) = parse_effect_token(token) {
            if effect_name == "tr" || effect_name == "transition" {
                let params = parse_params(&effect_value);
                if !params.is_empty() {
                    transition_sec = params[0].max(0.0);
                }
                continue;
            }
            
            match effect_name.as_str() {
                "rv" | "reverb" | "dl" | "delay" | "a" | "amplitude" | "p" | "pan" => {
                    // Check for duplicates
                    if seen_effects.contains(&effect_name) {
                        println!("[PARSER ERROR] Master: Effect '{}' specified multiple times - using first occurrence only", 
                                 effect_name);
                        continue;
                    }
                    seen_effects.insert(effect_name.clone());
                    
                    let params = parse_params(&effect_value);
                    if DEBUG_PARSER {
                        println!("[PARSER]     Master effect: {} with params {:?}", effect_name, params);
                    }
                    master_effects.push((effect_name, params));
                }
                _ => {
                    println!("[PARSER ERROR] Effect '{}' cannot be applied to master bus. Master only supports: a, p, rv, dl, cl, tr", effect_name);
                }
            }
        }
    }
    
    if should_clear || !master_effects.is_empty() || transition_sec > 0.0 {
        return CellAction::MasterEffects {
            clear_first: should_clear,
            transition_sec,
            effects: master_effects,
        };
    }

    CellAction::SlowRelease
}

fn parse_effect_token(token: &str) -> Option<(String, String)> {
    let token_lower = token.to_lowercase();
    if token_lower == "clear" || token_lower == "cl" {
        return Some((token_lower, String::new()));
    }
    
    if token_lower.starts_with("clear:") || token_lower.starts_with("cl:") {
        if let Some(colon_pos) = token.find(':') {
            let name = token[..colon_pos].to_lowercase();
            let value = token[colon_pos + 1..].to_string();
            return Some((name, value));
        }
    }
    
    if let Some(colon_pos) = token.find(':') {
        let name = token[..colon_pos].to_lowercase();
        let value = token[colon_pos + 1..].to_string();
        Some((name, value))
    } else {
        None
    }
}

fn parse_params(params_str: &str) -> Vec<f32> {
    params_str.split('\'')
        .filter_map(|s| s.parse::<f32>().ok())
        .collect()
}

fn apply_effect_to_state(
    state: &mut EffectState,
    effect_name: &str,
    effect_value: &str,
    transition_sec: &mut f32,
    clear_effects: &mut bool,
) {
    let params = parse_params(effect_value);

    if DEBUG_EFFECTS {
        println!("[EFFECT] Applying '{}' with params {:?}", effect_name, params);
    }

    match effect_name {
        "a" | "amplitude" => {
            if !params.is_empty() {
                state.amplitude = params[0].clamp(0.0, 1.0);
            }
        }
        "p" | "pan" => {
            if !params.is_empty() {
                state.pan = params[0].clamp(-1.0, 1.0);
            }
        }
        "v" | "vibrato" => {
            if params.len() >= 2 {
                state.vibrato_rate_hz = params[0].max(0.0);
                state.vibrato_depth_semitones = params[1].max(0.0);
            }
        }
        "t" | "tremolo" => {
            if params.len() >= 2 {
                state.tremolo_rate_hz = params[0].max(0.0);
                state.tremolo_depth = params[1].clamp(0.0, 1.0);
            }
        }
        "b" | "bitcrush" => {
            if !params.is_empty() {
                state.bitcrush_bits = (params[0] as u8).clamp(1, 16);
            }
        }
        "d" | "distortion" => {
            if !params.is_empty() {
                state.distortion_amount = params[0].clamp(0.0, 1.0);
            }
        }
        "tr" | "transition" => {
            if !params.is_empty() {
                *transition_sec = params[0].max(0.0);
            }
        }
        "cl" | "clear" => {
            *clear_effects = true;
            if !params.is_empty() {
                *transition_sec = params[0].max(0.0);
            }
        }
        _ => {
            println!("[EFFECT] Unknown effect: {}", effect_name);
        }
    }
}

// ============================================
// CHANNEL IMPLEMENTATION
// ============================================
impl Channel {
    fn trigger_note(
        &mut self,
        freq_hz: f32,
        instrument_id: usize,
        instrument_params: Vec<f32>,
        new_effects: EffectState,
        transition_sec: f32,
        clear_effects: bool,
    ) {
        if DEBUG_CHANNELS {
            println!("[CHANNEL {}] Triggering note: {:.2} Hz, inst {}, transition {:.2}s, clear: {}",
                     self.id, freq_hz, instrument_id, transition_sec, clear_effects);
        }

        let is_smooth_transition = transition_sec > 0.0 && self.active;
        
        if is_smooth_transition {
            // Smooth glide
            self.slide_active = true;
            self.slide_start_freq = self.frequency_hz;
            self.slide_target_freq = freq_hz;
            self.slide_duration_sec = transition_sec;
            self.slide_elapsed_sec = 0.0;
            
            if instrument_id != self.instrument_id {
                self.crossfade_active = true;
                self.crossfade_from_inst = self.instrument_id;
                self.crossfade_to_inst = instrument_id;
                self.instrument_id = instrument_id;
            }
            
            if !instrument_params.is_empty() {
                self.instrument_params = instrument_params;
            }
        } else {
            // Normal retrigger
            self.active = true;
            self.frequency_hz = freq_hz;
            self.instrument_id = instrument_id;
            self.instrument_params = instrument_params;
            self.phase = 0.0;
            self.envelope_state = EnvelopeState::Attack;
            self.envelope_level = 0.0;
            self.time_samples = 0;
            
            let inst = &INSTRUMENTS[instrument_id];
            self.attack_time_sec = inst.attack_sec;
            self.release_time_sec = inst.release_sec;
            
            self.slide_active = false;
            self.crossfade_active = false;
        }

        // Handle effects
        if clear_effects {
            let default_state = EffectState::default();
            
            if transition_sec > 0.0 {
                self.target_effects = Some(default_state);
                self.transition = Some(Transition {
                    duration_samples: (transition_sec * SAMPLE_RATE as f32) as u32,
                    elapsed_samples: 0,
                    start_state: self.current_effects.clone(),
                });
            } else {
                self.current_effects = default_state;
                merge_effects(&mut self.current_effects, &new_effects);
                self.target_effects = None;
                self.transition = None;
            }
        } else {
            if transition_sec > 0.0 {
                self.target_effects = Some(new_effects);
                self.transition = Some(Transition {
                    duration_samples: (transition_sec * SAMPLE_RATE as f32) as u32,
                    elapsed_samples: 0,
                    start_state: self.current_effects.clone(),
                });
            } else {
                self.current_effects = new_effects;
                self.target_effects = None;
                self.transition = None;
            }
        }
    }

    // NEW: Trigger pitchless instrument (like noise)
    fn trigger_pitchless(
        &mut self,
        instrument_id: usize,
        instrument_params: Vec<f32>,
        new_effects: EffectState,
        transition_sec: f32,
        clear_effects: bool,
    ) {
        // Pitchless instruments use a dummy frequency
        self.trigger_note(440.0, instrument_id, instrument_params, new_effects, transition_sec, clear_effects);
    }

    fn release(&mut self, release_time_sec: f32) {
        if self.active && self.envelope_state != EnvelopeState::Release {
            if DEBUG_CHANNELS {
                println!("[CHANNEL {}] Releasing note (release time: {:.3}s)", self.id, release_time_sec);
            }
            self.envelope_state = EnvelopeState::Release;
            self.release_time_sec = release_time_sec;
            self.release_start_level = self.envelope_level;
            self.release_start_time = self.time_samples;
        }
    }

    fn update_effects(&mut self, new_effects: EffectState, transition_sec: f32, clear_first: bool) {
        if DEBUG_CHANNELS {
            println!("[CHANNEL {}] Updating effects, transition {:.2}s, clear: {}",
                     self.id, transition_sec, clear_first);
        }

        if clear_first {
            let default_state = EffectState::default();
            
            if transition_sec > 0.0 {
                self.target_effects = Some(default_state);
                self.transition = Some(Transition {
                    duration_samples: (transition_sec * SAMPLE_RATE as f32) as u32,
                    elapsed_samples: 0,
                    start_state: self.current_effects.clone(),
                });
            } else {
                self.current_effects = default_state;
                self.target_effects = None;
                self.transition = None;
            }
        } else {
            let mut target_state = self.current_effects.clone();
            
            if new_effects.amplitude != EffectState::default().amplitude {
                target_state.amplitude = new_effects.amplitude;
            }
            if new_effects.pan != EffectState::default().pan {
                target_state.pan = new_effects.pan;
            }
            if new_effects.vibrato_rate_hz != EffectState::default().vibrato_rate_hz {
                target_state.vibrato_rate_hz = new_effects.vibrato_rate_hz;
                target_state.vibrato_depth_semitones = new_effects.vibrato_depth_semitones;
            }
            if new_effects.tremolo_rate_hz != EffectState::default().tremolo_rate_hz {
                target_state.tremolo_rate_hz = new_effects.tremolo_rate_hz;
                target_state.tremolo_depth = new_effects.tremolo_depth;
            }
            if new_effects.bitcrush_bits != EffectState::default().bitcrush_bits {
                target_state.bitcrush_bits = new_effects.bitcrush_bits;
            }
            if new_effects.distortion_amount != EffectState::default().distortion_amount {
                target_state.distortion_amount = new_effects.distortion_amount;
            }
            
            if transition_sec > 0.0 {
                self.target_effects = Some(target_state);
                self.transition = Some(Transition {
                    duration_samples: (transition_sec * SAMPLE_RATE as f32) as u32,
                    elapsed_samples: 0,
                    start_state: self.current_effects.clone(),
                });
            } else {
                self.current_effects = target_state;
                self.target_effects = None;
                self.transition = None;
            }
        }
    }

    fn render_sample(&mut self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        // Update transition
        if let Some(ref mut trans) = self.transition {
            trans.elapsed_samples += 1;
            let progress = (trans.elapsed_samples as f32 / trans.duration_samples as f32).clamp(0.0, 1.0);

            if let Some(ref target) = self.target_effects {
                self.current_effects.amplitude = lerp(trans.start_state.amplitude, target.amplitude, progress);
                self.current_effects.pan = lerp(trans.start_state.pan, target.pan, progress);
                self.current_effects.vibrato_rate_hz = lerp(trans.start_state.vibrato_rate_hz, target.vibrato_rate_hz, progress);
                self.current_effects.vibrato_depth_semitones = lerp(trans.start_state.vibrato_depth_semitones, target.vibrato_depth_semitones, progress);
                self.current_effects.tremolo_rate_hz = lerp(trans.start_state.tremolo_rate_hz, target.tremolo_rate_hz, progress);
                self.current_effects.tremolo_depth = lerp(trans.start_state.tremolo_depth, target.tremolo_depth, progress);
                self.current_effects.distortion_amount = lerp(trans.start_state.distortion_amount, target.distortion_amount, progress);
                let bitcrush_float = lerp(trans.start_state.bitcrush_bits as f32, target.bitcrush_bits as f32, progress);
                self.current_effects.bitcrush_bits = bitcrush_float.round() as u8;
            }

            if progress >= 1.0 {
                if let Some(target) = self.target_effects.take() {
                    self.current_effects = target;
                }
                self.transition = None;
            }
        }
        
        // Update slide
        if self.slide_active {
            self.slide_elapsed_sec += 1.0 / SAMPLE_RATE as f32;
            let slide_progress = (self.slide_elapsed_sec / self.slide_duration_sec).clamp(0.0, 1.0);
            
            self.frequency_hz = lerp(self.slide_start_freq, self.slide_target_freq, slide_progress);
            
            if slide_progress >= 1.0 {
                self.slide_active = false;
                self.crossfade_active = false;
            }
        }

        // Vibrato
        let vibrato_mult = if self.current_effects.vibrato_rate_hz > 0.0 {
            let lfo = self.current_effects.vibrato_phase.sin();
            2.0_f32.powf(lfo * self.current_effects.vibrato_depth_semitones / 12.0)
        } else {
            1.0
        };

        let modulated_freq = self.frequency_hz * vibrato_mult;
        self.phase += 2.0 * PI * modulated_freq / SAMPLE_RATE as f32;
        while self.phase >= 2.0 * PI {
            self.phase -= 2.0 * PI;
        }

        if self.current_effects.vibrato_rate_hz > 0.0 {
            self.current_effects.vibrato_phase += 2.0 * PI * self.current_effects.vibrato_rate_hz / SAMPLE_RATE as f32;
            while self.current_effects.vibrato_phase >= 2.0 * PI {
                self.current_effects.vibrato_phase -= 2.0 * PI;
            }
        }

        // Generate sample
        let mut sample = if self.crossfade_active {
            let crossfade_progress = (self.slide_elapsed_sec / self.slide_duration_sec).clamp(0.0, 1.0);
            
            let inst_a = &INSTRUMENTS[self.crossfade_from_inst];
            let inst_b = &INSTRUMENTS[self.crossfade_to_inst];
            
            let sample_a = generate_sample(
                inst_a.inst_type,
                self.phase,
                &self.instrument_params,
                &mut self.rng_state,
            );
            
            let sample_b = generate_sample(
                inst_b.inst_type,
                self.phase,
                &self.instrument_params,
                &mut self.rng_state,
            );
            
            let fade_out = (1.0 - crossfade_progress).sqrt();
            let fade_in = crossfade_progress.sqrt();
            sample_a * fade_out + sample_b * fade_in
        } else {
            let inst = &INSTRUMENTS[self.instrument_id];
            generate_sample(
                inst.inst_type,
                self.phase,
                &self.instrument_params,
                &mut self.rng_state,
            )
        };

        // Envelope
        self.update_envelope();
        sample *= self.envelope_level;

        // Effects
        sample = self.apply_effects(sample);

        // Pan
        let amplitude = self.current_effects.amplitude;
        let pan = self.current_effects.pan;

        let pan_left = ((1.0 - pan) * 0.5).sqrt();
        let pan_right = ((1.0 + pan) * 0.5).sqrt();

        let left = sample * amplitude * pan_left;
        let right = sample * amplitude * pan_right;

        self.time_samples += 1;

        if self.envelope_state == EnvelopeState::Release && self.envelope_level < 0.001 {
            self.active = false;
            if DEBUG_CHANNELS {
                println!("[CHANNEL {}] Deactivated (envelope finished)", self.id);
            }
        }

        (left, right)
    }

    fn update_envelope(&mut self) {
        match self.envelope_state {
            EnvelopeState::Idle => {
                self.envelope_level = 0.0;
            }
            EnvelopeState::Attack => {
                let attack_samples = (self.attack_time_sec * SAMPLE_RATE as f32) as u64;
                if attack_samples > 0 {
                    self.envelope_level = (self.time_samples as f32 / attack_samples as f32).min(1.0);
                    if self.envelope_level >= 1.0 {
                        self.envelope_state = EnvelopeState::Sustain;
                    }
                } else {
                    self.envelope_level = 1.0;
                    self.envelope_state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                self.envelope_level = 1.0;
            }
            EnvelopeState::Release => {
                let release_samples = (self.release_time_sec * SAMPLE_RATE as f32) as u32;
                let samples_since_release = self.time_samples.saturating_sub(self.release_start_time);

                if release_samples > 0 {
                    let progress = (samples_since_release as f32 / release_samples as f32).min(1.0);
                    self.envelope_level = self.release_start_level * (1.0 - progress);
                } else {
                    self.envelope_level = 0.0;
                }
            }
        }
    }

    fn apply_effects(&mut self, mut sample: f32) -> f32 {
        // Tremolo
        if self.current_effects.tremolo_rate_hz > 0.0 {
            let lfo = self.current_effects.tremolo_phase.sin();
            let amp_mod = 1.0 - self.current_effects.tremolo_depth * (1.0 - lfo) / 2.0;
            sample *= amp_mod;
            self.current_effects.tremolo_phase += 2.0 * PI * self.current_effects.tremolo_rate_hz / SAMPLE_RATE as f32;
        }

        // Bitcrush
        if self.current_effects.bitcrush_bits < 16 {
            let levels = 2.0_f32.powi(self.current_effects.bitcrush_bits as i32);
            sample = (sample * levels).round() / levels;
        }

        // Distortion
        if self.current_effects.distortion_amount > 0.0 {
            let drive = 1.0 + self.current_effects.distortion_amount * 10.0;
            let x = sample * drive;
            sample = x / (1.0 + x.abs());
        }

        sample
    }
}

fn merge_effects(current: &mut EffectState, new: &EffectState) {
    if new.amplitude != 1.0 {
        current.amplitude = new.amplitude;
    }
    if new.pan != 0.0 {
        current.pan = new.pan;
    }
    if new.vibrato_rate_hz != 0.0 {
        current.vibrato_rate_hz = new.vibrato_rate_hz;
        current.vibrato_depth_semitones = new.vibrato_depth_semitones;
    }
    if new.tremolo_rate_hz != 0.0 {
        current.tremolo_rate_hz = new.tremolo_rate_hz;
        current.tremolo_depth = new.tremolo_depth;
    }
    if new.bitcrush_bits != 16 {
        current.bitcrush_bits = new.bitcrush_bits;
    }
    if new.distortion_amount != 0.0 {
        current.distortion_amount = new.distortion_amount;
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ============================================
// PLAYBACK ENGINE
// ============================================
impl PlaybackEngine {
    fn new(song: SongData) -> Self {
        let samples_per_row = (TICK_DURATION_SEC * SAMPLE_RATE as f32) as u32;
        let channels: Vec<Channel> = (0..NUM_CHANNELS).map(|id| Channel::new(id)).collect();

        println!("[ENGINE] Initialized with {} channels, {} samples per row ({:.2}s per row)",
                 NUM_CHANNELS, samples_per_row, TICK_DURATION_SEC);

        Self {
            song,
            current_row: 0,
            samples_in_current_row: 0,
            samples_per_row,
            channels,
            master_bus: MasterBus::new(),
        }
    }

    fn advance_row(&mut self) {
        if self.current_row >= self.song.rows.len() {
            return;
        }

        if DEBUG_PLAYBACK && self.current_row < self.song.raw_lines.len() {
            println!("Row {}", self.current_row);
            println!("{}\n", self.song.raw_lines[self.current_row]);
        }

        let row = self.song.rows[self.current_row].clone();

        for (channel_idx, action) in row.iter().enumerate() {
            if channel_idx >= NUM_CHANNELS {
                break;
            }

            match action {
                CellAction::TriggerNote { pitch, instrument_id, instrument_params, effects, transition_sec, clear_effects } => {
                    let freq = pitch_to_frequency(pitch);
                    self.channels[channel_idx].trigger_note(
                        freq,
                        *instrument_id,
                        instrument_params.clone(),
                        effects.clone(),
                        *transition_sec,
                        *clear_effects,
                    );
                }
                CellAction::TriggerPitchless { instrument_id, instrument_params, effects, transition_sec, clear_effects } => {
                    self.channels[channel_idx].trigger_pitchless(
                        *instrument_id,
                        instrument_params.clone(),
                        effects.clone(),
                        *transition_sec,
                        *clear_effects,
                    );
                }
                CellAction::Sustain => {
                    if self.channels[channel_idx].active {
                        self.channels[channel_idx].envelope_state = EnvelopeState::Sustain;
                        self.channels[channel_idx].envelope_level = 1.0;
                    }
                }
                CellAction::SustainWithEffects { effects, transition_sec, clear_first } => {
                    if self.channels[channel_idx].active {
                        self.channels[channel_idx].envelope_state = EnvelopeState::Sustain;
                        self.channels[channel_idx].envelope_level = 1.0;
                    }
                    self.channels[channel_idx].update_effects(effects.clone(), *transition_sec, *clear_first);
                }
                CellAction::FastRelease => {
                    self.channels[channel_idx].release(FAST_RELEASE_SEC);
                }
                CellAction::SlowRelease => {
                    self.channels[channel_idx].release(DEFAULT_RELEASE_SEC);
                }
                CellAction::ChangeEffects { effects, transition_sec, clear_first } => {
                    self.channels[channel_idx].update_effects(effects.clone(), *transition_sec, *clear_first);
                }
                CellAction::MasterEffects { clear_first, transition_sec, effects } => {
                    if *clear_first {
                        self.master_bus.clear_effects(*transition_sec);
                    }
                    for (effect_name, params) in effects {
                        self.master_bus.apply_effect(effect_name, params, *transition_sec);
                    }
                }
            }
        }

        self.current_row += 1;
        self.samples_in_current_row = 0;
    }

    fn process_frame(&mut self, output: &mut [f32]) {
        for sample_pair in output.chunks_mut(2) {
            if self.samples_in_current_row >= self.samples_per_row {
                self.advance_row();
            }

            let mut left = 0.0;
            let mut right = 0.0;

            for channel in &mut self.channels {
                if channel.active {
                    let (l, r) = channel.render_sample();
                    left += l;
                    right += r;
                }
            }

            (left, right) = self.master_bus.process(left, right);

            sample_pair[0] = left.clamp(-1.0, 1.0);
            sample_pair[1] = right.clamp(-1.0, 1.0);

            self.samples_in_current_row += 1;
        }
    }
}

// ============================================
// MASTER BUS IMPLEMENTATION
// ============================================
impl MasterBus {
    fn clear_effects(&mut self, transition_sec: f32) {
        if DEBUG_MASTER {
            println!("[MASTER] Clearing all effects (transition: {:.2}s)", transition_sec);
        }
        
        if transition_sec > 0.0 {
            self.start_amplitude = self.amplitude;
            self.start_pan = self.pan;
            self.start_reverb_room_size = self.reverb_room_size;
            self.start_reverb_mix = self.reverb_mix;
            self.start_delay_time_samples = self.delay_time_samples;
            self.start_delay_feedback = self.delay_feedback;
            self.start_reverb_enabled = self.reverb_enabled;
            self.start_delay_enabled = self.delay_enabled;
            
            self.target_amplitude = 1.0;
            self.target_pan = 0.0;
            self.target_reverb_room_size = 0.5;
            self.target_reverb_mix = 0.0;
            self.target_delay_time_samples = SAMPLE_RATE / 4;
            self.target_delay_feedback = 0.0;
            self.target_reverb_enabled = false;
            self.target_delay_enabled = false;
            
            self.transition_active = true;
            self.transition_duration_samples = (transition_sec * SAMPLE_RATE as f32) as u32;
            self.transition_elapsed_samples = 0;
        } else {
            self.amplitude = 1.0;
            self.pan = 0.0;
            self.reverb_enabled = false;
            self.delay_enabled = false;
            self.transition_active = false;
        }
    }
    
    fn apply_effect(&mut self, effect_name: &str, params: &[f32], transition_sec: f32) {
        if DEBUG_MASTER {
            println!("[MASTER] Applying effect '{}' with params {:?} (transition: {:.2}s)", 
                     effect_name, params, transition_sec);
        }
        
        match effect_name {
            "a" | "amplitude" => {
                if !params.is_empty() {
                    let new_amp = params[0].clamp(0.0, 1.0);
                    
                    if transition_sec > 0.0 {
                        if !self.transition_active {
                            self.start_amplitude = self.amplitude;
                            self.start_pan = self.pan;
                            self.start_reverb_room_size = self.reverb_room_size;
                            self.start_reverb_mix = self.reverb_mix;
                            self.start_delay_time_samples = self.delay_time_samples;
                            self.start_delay_feedback = self.delay_feedback;
                            self.start_reverb_enabled = self.reverb_enabled;
                            self.start_delay_enabled = self.delay_enabled;
                        }
                        
                        self.target_amplitude = new_amp;
                        
                        self.transition_active = true;
                        self.transition_duration_samples = (transition_sec * SAMPLE_RATE as f32) as u32;
                        self.transition_elapsed_samples = 0;
                    } else {
                        self.amplitude = new_amp;
                    }
                    
                    if DEBUG_MASTER {
                        println!("[MASTER] Amplitude: {:.2}", new_amp);
                    }
                }
            }
            "p" | "pan" => {
                if !params.is_empty() {
                    let new_pan = params[0].clamp(-1.0, 1.0);
                    
                    if transition_sec > 0.0 {
                        if !self.transition_active {
                            self.start_amplitude = self.amplitude;
                            self.start_pan = self.pan;
                            self.start_reverb_room_size = self.reverb_room_size;
                            self.start_reverb_mix = self.reverb_mix;
                            self.start_delay_time_samples = self.delay_time_samples;
                            self.start_delay_feedback = self.delay_feedback;
                            self.start_reverb_enabled = self.reverb_enabled;
                            self.start_delay_enabled = self.delay_enabled;
                        }
                        
                        self.target_pan = new_pan;
                        
                        self.transition_active = true;
                        self.transition_duration_samples = (transition_sec * SAMPLE_RATE as f32) as u32;
                        self.transition_elapsed_samples = 0;
                    } else {
                        self.pan = new_pan;
                    }
                    
                    if DEBUG_MASTER {
                        println!("[MASTER] Pan: {:.2}", new_pan);
                    }
                }
            }
            "rv" | "reverb" => {
                if params.len() >= 2 {
                    let new_room = params[0].clamp(0.0, 1.0);
                    let new_mix = params[1].clamp(0.0, 1.0);
                    
                    if transition_sec > 0.0 {
                        if !self.transition_active {
                            self.start_amplitude = self.amplitude;
                            self.start_pan = self.pan;
                            self.start_reverb_room_size = self.reverb_room_size;
                            self.start_reverb_mix = self.reverb_mix;
                            self.start_delay_time_samples = self.delay_time_samples;
                            self.start_delay_feedback = self.delay_feedback;
                            self.start_reverb_enabled = self.reverb_enabled;
                            self.start_delay_enabled = self.delay_enabled;
                        }
                        
                        self.target_reverb_room_size = new_room;
                        self.target_reverb_mix = new_mix;
                        self.target_reverb_enabled = true;
                        
                        self.transition_active = true;
                        self.transition_duration_samples = (transition_sec * SAMPLE_RATE as f32) as u32;
                        self.transition_elapsed_samples = 0;
                    } else {
                        self.reverb_enabled = true;
                        self.reverb_room_size = new_room;
                        self.reverb_mix = new_mix;
                    }
                    
                    if DEBUG_MASTER {
                        println!("[MASTER] Reverb: room {:.2}, mix {:.2}", new_room, new_mix);
                    }
                }
            }
            "dl" | "delay" => {
                if params.len() >= 2 {
                    let new_time_samples = (params[0] * SAMPLE_RATE as f32) as u32;
                    let new_feedback = params[1].clamp(0.0, 0.95);
                    
                    if transition_sec > 0.0 {
                        if !self.transition_active {
                            self.start_amplitude = self.amplitude;
                            self.start_pan = self.pan;
                            self.start_reverb_room_size = self.reverb_room_size;
                            self.start_reverb_mix = self.reverb_mix;
                            self.start_delay_time_samples = self.delay_time_samples;
                            self.start_delay_feedback = self.delay_feedback;
                            self.start_reverb_enabled = self.reverb_enabled;
                            self.start_delay_enabled = self.delay_enabled;
                        }
                        
                        self.target_delay_time_samples = new_time_samples;
                        self.target_delay_feedback = new_feedback;
                        self.target_delay_enabled = true;
                        
                        self.transition_active = true;
                        self.transition_duration_samples = (transition_sec * SAMPLE_RATE as f32) as u32;
                        self.transition_elapsed_samples = 0;
                    } else {
                        self.delay_enabled = true;
                        self.delay_time_samples = new_time_samples;
                        self.delay_feedback = new_feedback;
                    }
                    
                    if DEBUG_MASTER {
                        println!("[MASTER] Delay: time {:.2}s, feedback {:.2}", params[0], new_feedback);
                    }
                }
            }
            _ => {
                println!("[MASTER] Unknown master effect: {}", effect_name);
            }
        }
    }
    
    fn process(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        if self.transition_active {
            self.transition_elapsed_samples += 1;
            let progress = (self.transition_elapsed_samples as f32 / self.transition_duration_samples as f32).clamp(0.0, 1.0);
            
            self.amplitude = lerp(self.start_amplitude, self.target_amplitude, progress);
            self.pan = lerp(self.start_pan, self.target_pan, progress);
            self.reverb_room_size = lerp(self.start_reverb_room_size, self.target_reverb_room_size, progress);
            self.reverb_mix = lerp(self.start_reverb_mix, self.target_reverb_mix, progress);
            self.delay_time_samples = lerp(self.start_delay_time_samples as f32, self.target_delay_time_samples as f32, progress) as u32;
            self.delay_feedback = lerp(self.start_delay_feedback, self.target_delay_feedback, progress);
            
            if progress >= 1.0 {
                self.reverb_enabled = self.target_reverb_enabled;
                self.delay_enabled = self.target_delay_enabled;
                self.transition_active = false;
            }
        }
        
        if self.reverb_enabled || (self.transition_active && self.reverb_mix > 0.001) {
            let delay_samples = (self.reverb_room_size * SAMPLE_RATE as f32 * 0.05) as usize;
            let delay_samples = delay_samples.min(self.reverb_buffer.len() - 1);

            let read_pos = (self.reverb_pos + self.reverb_buffer.len() - delay_samples) % self.reverb_buffer.len();
            let reverb_sample = self.reverb_buffer[read_pos];

            self.reverb_buffer[self.reverb_pos] = (left + right) * 0.5 + reverb_sample * 0.5;
            self.reverb_pos = (self.reverb_pos + 1) % self.reverb_buffer.len();

            let wet = reverb_sample * self.reverb_mix;
            let dry = 1.0 - self.reverb_mix;
            left = left * dry + wet;
            right = right * dry + wet;
        }

        if self.delay_enabled || (self.transition_active && self.delay_feedback > 0.001) {
            let delay_samples = self.delay_time_samples as usize;
            let delay_samples = delay_samples.min(self.delay_buffer_l.len() - 1);

            let read_pos = (self.delay_write_pos + self.delay_buffer_l.len() - delay_samples) % self.delay_buffer_l.len();

            let delayed_l = self.delay_buffer_l[read_pos];
            let delayed_r = self.delay_buffer_r[read_pos];

            self.delay_buffer_l[self.delay_write_pos] = left + delayed_l * self.delay_feedback;
            self.delay_buffer_r[self.delay_write_pos] = right + delayed_r * self.delay_feedback;

            left += delayed_l * 0.5;
            right += delayed_r * 0.5;

            self.delay_write_pos = (self.delay_write_pos + 1) % self.delay_buffer_l.len();
        }

        left *= self.amplitude;
        right *= self.amplitude;
        
        if self.pan != 0.0 {
            let pan_left = ((1.0 - self.pan) * 0.5).sqrt();
            let pan_right = ((1.0 + self.pan) * 0.5).sqrt();
            left *= pan_left;
            right *= pan_right;
        }

        (left, right)
    }
}

// ============================================
// MAIN
// ============================================
fn main() {
    println!("\n╔════════════════════════════════════════╗");
    println!("║   RUST TRACKER SYNTH v2.0 (FIXED)     ║");
    println!("╚════════════════════════════════════════╝\n");

    let song_source = if USE_FILE {
        println!("[MAIN] Loading song from file: {}", SONG_FILE_PATH);
        fs::read_to_string(SONG_FILE_PATH).expect("Failed to read song file")
    } else {
        println!("[MAIN] Using embedded song string");
        SONG_STRING.to_string()
    };

    let song_data = parse_song(&song_source);

    let total_duration_sec = song_data.rows.len() as f32 * TICK_DURATION_SEC;
    println!("[MAIN] Song duration: {:.2}s ({} rows)", total_duration_sec, song_data.rows.len());

    let engine = Arc::new(Mutex::new(PlaybackEngine::new(song_data)));
    let engine_clone = Arc::clone(&engine);

    println!("\n[AUDIO] Initializing miniaudio...");
    let audio_context = Context::new(&[], None).expect("Failed to create audio context");

    let mut device_config = DeviceConfig::new(DeviceType::Playback);
    device_config.playback_mut().set_format(Format::F32);
    device_config.playback_mut().set_channels(2);
    device_config.set_sample_rate(SAMPLE_RATE);
    device_config.set_period_size_in_frames(960);

    device_config.set_data_callback(move |_device, output_buffer, _input_buffer| {
        let samples = output_buffer.as_samples_mut::<f32>();
        if let Ok(mut eng) = engine_clone.lock() {
            eng.process_frame(samples);
        }
    });

    let audio_device = Device::new(Some(audio_context), &device_config)
        .expect("Failed to create audio device");

    println!("[AUDIO] Starting playback...");
    audio_device.start().expect("Failed to start audio device");

    println!("\n▶ PLAYING... (duration: {:.2}s)\n", total_duration_sec);

    thread::sleep(Duration::from_secs_f32(total_duration_sec + 2.0));

    println!("\n[MAIN] Playback finished!");
    println!("╔════════════════════════════════════════╗");
    println!("║   THANK YOU FOR LISTENING!             ║");
    println!("╚════════════════════════════════════════╝\n");
}

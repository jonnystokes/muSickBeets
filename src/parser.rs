// ============================================================================
// PARSER.RS - CSV Song File Parser
// ============================================================================
//
// This module parses CSV song files into playable data structures.
// It's designed to be VERY FORGIVING - sloppy input is handled gracefully.
//
// CSV FORMAT:
// - First row is the header (voice names) - skipped
// - Each subsequent row is one time step
// - Each column is a channel/voice
// - Cells contain commands for that channel at that time
//
// CELL SYNTAX:
// - ""         Empty = slow release (fade out)
// - "-"        Sustain = keep playing
// - "."        Fast release = quick fade to avoid pops
// - "c4 sine"  Note trigger = play C4 with sine wave
// - "a:0.5"    Effect change = set amplitude to 50%
// - "master rv:0.5'0.3"  Master effect = reverb on master bus
//
// ERROR HANDLING:
// The parser reports errors with line and column numbers, then continues
// parsing. This allows you to see ALL errors at once instead of fixing
// them one at a time. Invalid cells are treated as slow release.
// ============================================================================

use std::collections::HashSet;
use crate::effects::ChannelEffectState;
use crate::instruments::{find_instrument_by_name, get_instrument_by_id};
use crate::helper::{parse_pitch_to_frequency, FrequencyTable};

// ============================================================================
// DEBUG LEVELS
// ============================================================================

/// Debug output level for the parser
/// Configure this in main.rs to control how much output you see
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum DebugLevel {
    /// No debug output
    Off = 0,

    /// Basic info: parsing start/end, row counts, errors
    Basic = 1,

    /// Verbose: + channel activity, effect changes
    Verbose = 2,

    /// Detailed: + individual token parsing, all internal state
    Detailed = 3,
}

// ============================================================================
// PARSE ERROR
// ============================================================================

/// Represents a parsing error with location information
#[derive(Clone, Debug)]
pub struct ParseError {
    /// Line number in the original file (1-indexed for human readability)
    pub line_number: usize,

    /// Column number (channel index, 0-indexed)
    pub column_number: usize,

    /// The raw cell content that caused the error
    pub cell_content: String,

    /// Human-readable error message
    pub message: String,

    /// Whether parsing can continue (warning) or must stop (fatal)
    pub is_fatal: bool,
}

impl ParseError {
    /// Creates a new non-fatal error (warning)
    pub fn warning(line: usize, column: usize, cell: &str, message: String) -> Self {
        Self {
            line_number: line,
            column_number: column,
            cell_content: cell.to_string(),
            message,
            is_fatal: false,
        }
    }

    /// Creates a new fatal error
    pub fn fatal(line: usize, column: usize, cell: &str, message: String) -> Self {
        Self {
            line_number: line,
            column_number: column,
            cell_content: cell.to_string(),
            message,
            is_fatal: true,
        }
    }

    /// Formats the error for display
    pub fn format(&self) -> String {
        let error_type = if self.is_fatal { "ERROR" } else { "WARNING" };
        format!(
            "[{}] Line {}, Channel {}: {} (cell: '{}')",
            error_type, self.line_number, self.column_number, self.message, self.cell_content
        )
    }
}

// ============================================================================
// CELL ACTION
// ============================================================================
//
// CellAction represents what should happen on a channel during one time step.
// The parser converts cell text into CellActions, which the engine executes.
// ============================================================================

/// What action to take for a cell in the song
#[derive(Clone, Debug)]
pub enum CellAction {
    /// Trigger a pitched note (e.g., "c4 sine")
    TriggerNote {
        /// The pitch string (e.g., "c4", "f#3")
        pitch: String,

        /// Frequency in Hz (pre-calculated for performance)
        frequency_hz: f32,

        /// Instrument ID
        instrument_id: usize,

        /// Instrument parameters (e.g., trisaw shape, pulse width)
        instrument_parameters: Vec<f32>,

        /// Effect settings for this note
        effects: ChannelEffectState,

        /// Transition time in seconds (0 = instant)
        transition_seconds: f32,

        /// Whether to clear effects to default first
        clear_effects: bool,
    },

    /// Trigger a pitchless instrument (e.g., "noise a:0.5")
    TriggerPitchless {
        /// Instrument ID
        instrument_id: usize,

        /// Instrument parameters
        instrument_parameters: Vec<f32>,

        /// Effect settings
        effects: ChannelEffectState,

        /// Transition time in seconds
        transition_seconds: f32,

        /// Whether to clear effects first
        clear_effects: bool,
    },

    /// Keep playing the current sound
    Sustain,

    /// Keep playing but change effects (e.g., "- a:0.5")
    SustainWithEffects {
        /// New effect settings
        effects: ChannelEffectState,

        /// Transition time
        transition_seconds: f32,

        /// Whether to clear effects first
        clear_first: bool,
    },

    /// Quick fade out (50ms) to avoid pops
    FastRelease,

    /// Slow fade out (2 seconds default)
    SlowRelease,

    /// Change effects without retriggering (e.g., "a:0.5 p:-0.3")
    ChangeEffects {
        /// New effect settings
        effects: ChannelEffectState,

        /// Transition time
        transition_seconds: f32,

        /// Whether to clear effects first
        clear_first: bool,
    },

    /// Master bus effect command
    MasterEffects {
        /// Whether to clear master effects first
        clear_first: bool,

        /// Transition time
        transition_seconds: f32,

        /// List of effects to apply: (effect_name, parameters)
        effects: Vec<(String, Vec<f32>)>,
    },
}

// ============================================================================
// SONG DATA
// ============================================================================

/// Parsed song data ready for playback
pub struct SongData {
    /// Grid of cell actions: rows[row_index][channel_index]
    pub rows: Vec<Vec<CellAction>>,

    /// Original line content for each row (for debug display)
    pub raw_lines: Vec<String>,

    /// Any errors encountered during parsing
    pub errors: Vec<ParseError>,

    /// Number of channels detected
    pub channel_count: usize,
}

impl SongData {
    /// Returns true if there were any fatal errors
    pub fn has_fatal_errors(&self) -> bool {
        self.errors.iter().any(|e| e.is_fatal)
    }

    /// Returns the total duration in rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Prints all errors to stdout
    pub fn print_errors(&self) {
        for error in &self.errors {
            println!("{}", error.format());
        }
    }
}

// ============================================================================
// PARSER CONTEXT
// ============================================================================

/// Internal parser state
struct ParserContext<'a> {
    /// The frequency table for pitch lookups
    frequency_table: &'a FrequencyTable,

    /// Number of channels to parse
    channel_count: usize,

    /// Debug level
    debug_level: DebugLevel,

    /// Current line number (for error messages)
    current_line: usize,

    /// Current column/channel (for error messages)
    current_column: usize,

    /// Collected errors
    errors: Vec<ParseError>,

    /// Behavior for missing cells at end of row
    missing_cell_behavior: MissingCellBehavior,
}

/// What to do when a row has fewer cells than channels
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MissingCellBehavior {
    /// Treat missing cells as sustain (keep playing)
    Sustain,

    /// Treat missing cells as slow release (fade out)
    SlowRelease,
}

// ============================================================================
// MAIN PARSER FUNCTION
// ============================================================================

/// Parses a CSV song string into playable SongData
///
/// Parameters:
/// - song_text: The raw CSV content
/// - frequency_table: Pre-computed frequency table for pitch lookups
/// - channel_count: How many channels to parse
/// - missing_cell_behavior: What to do for missing cells
/// - debug_level: How much debug output to print
pub fn parse_song(
    song_text: &str,
    frequency_table: &FrequencyTable,
    channel_count: usize,
    missing_cell_behavior: MissingCellBehavior,
    debug_level: DebugLevel,
) -> SongData {
    if debug_level >= DebugLevel::Basic {
        println!("\n[PARSER] ========== PARSING SONG ==========");
    }

    let mut context = ParserContext {
        frequency_table,
        channel_count,
        debug_level,
        current_line: 0,
        current_column: 0,
        errors: Vec::new(),
        missing_cell_behavior,
    };

    let mut rows: Vec<Vec<CellAction>> = Vec::new();
    let mut raw_lines: Vec<String> = Vec::new();
    let mut is_first_data_row = true;

    for (line_index, line) in song_text.lines().enumerate() {
        context.current_line = line_index + 1; // 1-indexed for humans

        // Strip comments from the line
        let line_without_comments = strip_comments(line);
        let trimmed_line = line_without_comments.trim();

        // Skip empty lines
        if trimmed_line.is_empty() {
            if debug_level >= DebugLevel::Detailed {
                println!("[PARSER] Line {}: Skipping empty/comment line", context.current_line);
            }
            continue;
        }

        // Skip header row (first non-empty line)
        if is_first_data_row {
            is_first_data_row = false;
            if debug_level >= DebugLevel::Verbose {
                println!("[PARSER] Line {}: Skipping header: '{}'", context.current_line, trimmed_line);
            }
            continue;
        }

        // Store raw line for debug display
        raw_lines.push(trimmed_line.to_string());

        if debug_level >= DebugLevel::Verbose {
            println!("[PARSER] Row {}: '{}'", rows.len(), trimmed_line);
        }

        // Split into cells
        let cells: Vec<&str> = trimmed_line.split(',').collect();
        let mut row_actions: Vec<CellAction> = Vec::new();

        // Parse each cell
        for channel_index in 0..channel_count {
            context.current_column = channel_index;

            let cell_content = if channel_index < cells.len() {
                cells[channel_index].trim()
            } else {
                // Missing cell - use configured behavior
                if debug_level >= DebugLevel::Detailed {
                    println!(
                        "[PARSER]   Channel {}: MISSING (using {:?})",
                        channel_index, context.missing_cell_behavior
                    );
                }
                match context.missing_cell_behavior {
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

            if debug_level >= DebugLevel::Detailed {
                println!("[PARSER]   Channel {}: '{}'", channel_index, cell_content);
            }

            let action = parse_cell(cell_content, &mut context);
            row_actions.push(action);
        }

        // Warn about extra cells
        if cells.len() > channel_count {
            context.errors.push(ParseError::warning(
                context.current_line,
                channel_count,
                "",
                format!(
                    "Row has {} cells but only {} channels configured. Extra cells ignored.",
                    cells.len(),
                    channel_count
                ),
            ));
        }

        rows.push(row_actions);
    }

    if debug_level >= DebugLevel::Basic {
        println!(
            "[PARSER] ========== PARSING COMPLETE: {} rows, {} errors ==========\n",
            rows.len(),
            context.errors.len()
        );
    }

    SongData {
        rows,
        raw_lines,
        errors: context.errors,
        channel_count,
    }
}

// ============================================================================
// COMMENT STRIPPING
// ============================================================================

/// Removes comments from a line
/// Supports // comments and # comments (but preserves # in sharp notes like c#4)
fn strip_comments(line: &str) -> &str {
    // Handle // comments (always a comment)
    if let Some(slash_position) = line.find("//") {
        return &line[..slash_position];
    }

    // Handle # comments (but preserve sharp notes)
    if let Some(hash_position) = line.find('#') {
        // Check if the # is a sharp note modifier
        let is_sharp_note = if hash_position > 0 {
            let char_before = line.as_bytes()[hash_position - 1] as char;
            matches!(char_before.to_ascii_lowercase(), 'a'..='g')
        } else {
            false
        };

        if !is_sharp_note {
            return &line[..hash_position];
        }
    }

    line
}

// ============================================================================
// CELL PARSING
// ============================================================================

/// Parses a single cell into a CellAction
fn parse_cell(cell: &str, context: &mut ParserContext) -> CellAction {
    let cell = cell.trim();

    // Empty cell = Slow Release
    if cell.is_empty() {
        return CellAction::SlowRelease;
    }

    // Sustain (plain hyphen)
    if cell == "-" {
        return CellAction::Sustain;
    }

    // Fast Release (period)
    if cell == "." {
        return CellAction::FastRelease;
    }

    // Split into tokens (handles multiple spaces)
    let tokens: Vec<&str> = cell.split_whitespace().collect();
    if tokens.is_empty() {
        return CellAction::SlowRelease;
    }

    // Check for sustain with effects: "- a:0.5"
    if tokens[0] == "-" && tokens.len() > 1 {
        return parse_sustain_with_effects(&tokens[1..], context);
    }

    // Determine what kind of cell this is by looking at the first token
    let first_token = tokens[0];
    let first_char = first_token.chars().next().unwrap().to_ascii_lowercase();

    // Is it a note? (starts with a-g)
    let is_note = matches!(first_char, 'a'..='g');

    if is_note {
        // Note trigger: "c4 sine a:0.8"
        return parse_note_trigger(&tokens, context);
    }

    // Check if first token is an instrument name
    if let Some(instrument_id) = find_instrument_by_name(first_token) {
        if instrument_id == 0 {
            // Instrument 0 = master bus effects
            return parse_master_effects(&tokens, context);
        } else {
            // Check if this instrument requires a pitch
            if let Some(instrument) = get_instrument_by_id(instrument_id) {
                if !instrument.requires_pitch {
                    // Pitchless instrument (like noise)
                    return parse_pitchless_trigger(&tokens, context);
                } else {
                    // Requires pitch but none given
                    context.errors.push(ParseError::warning(
                        context.current_line,
                        context.current_column,
                        cell,
                        format!(
                            "Instrument '{}' requires a note (e.g., 'c4 {}')",
                            instrument.name, instrument.name
                        ),
                    ));
                    return CellAction::SlowRelease;
                }
            }
        }
    }

    // Check if it's a master-only effect
    if is_master_effect(first_token) {
        return parse_master_effects(&tokens, context);
    }

    // Otherwise, it's a channel effect change
    parse_effect_change(&tokens, context)
}

/// Parses "- a:0.5 tr:2" (sustain with effect changes)
fn parse_sustain_with_effects(tokens: &[&str], context: &mut ParserContext) -> CellAction {
    let (effects, transition_seconds, clear_first) = parse_effect_tokens(tokens, context);

    CellAction::SustainWithEffects {
        effects,
        transition_seconds,
        clear_first,
    }
}

/// Parses a note trigger like "c4 sine a:0.8"
fn parse_note_trigger(tokens: &[&str], context: &mut ParserContext) -> CellAction {
    let pitch = tokens[0].to_string();

    // Look up frequency from table
    let frequency_hz = match parse_pitch_to_frequency(&pitch, context.frequency_table) {
        Some(freq) => freq,
        None => {
            context.errors.push(ParseError::warning(
                context.current_line,
                context.current_column,
                &pitch,
                format!("Invalid pitch '{}'. Using A4 (440 Hz).", pitch),
            ));
            440.0
        }
    };

    let mut instrument_id = 1; // Default to sine
    let mut instrument_parameters: Vec<f32> = Vec::new();
    let mut seen_effects: HashSet<String> = HashSet::new();

    // First pass: find clear flag and instrument
    let mut clear_effects = false;
    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_effects = true;
        }

        // Check for instrument (without colon)
        if !token.contains(':') {
            if let Some(id) = find_instrument_by_name(token) {
                if id == 0 {
                    context.errors.push(ParseError::warning(
                        context.current_line,
                        context.current_column,
                        token,
                        "Cannot play notes on 'master'. Use a playable instrument.".to_string(),
                    ));
                    return CellAction::SlowRelease;
                }
                instrument_id = id;
            }
        }
    }

    // Second pass: parse instrument params and effects
    let mut effects = ChannelEffectState::default();
    effects.initialize_chorus_buffer(48000); // Will be re-initialized if needed
    let mut transition_seconds = 0.0;

    for token in &tokens[1..] {
        let token_lower = token.to_lowercase();

        // Skip clear token
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }

        // Skip if it's a standalone instrument name (already handled)
        if !token.contains(':') {
            if find_instrument_by_name(token).is_some() {
                continue;
            }
            // Unknown standalone token
            context.errors.push(ParseError::warning(
                context.current_line,
                context.current_column,
                token,
                format!("Unrecognized token '{}' - ignoring", token),
            ));
            continue;
        }

        // Parse colon-separated token
        if let Some(colon_pos) = token.find(':') {
            let prefix = &token[..colon_pos].to_lowercase();
            let value_str = &token[colon_pos + 1..];

            // Check if it's an instrument with parameters (e.g., "trisaw:0.5")
            if let Some(id) = find_instrument_by_name(prefix) {
                if id == 0 {
                    context.errors.push(ParseError::warning(
                        context.current_line,
                        context.current_column,
                        token,
                        "Cannot play notes on 'master'.".to_string(),
                    ));
                    return CellAction::SlowRelease;
                }
                instrument_id = id;
                instrument_parameters = parse_parameter_list(value_str);
                continue;
            }

            // It's an effect
            if seen_effects.contains(prefix) {
                context.errors.push(ParseError::warning(
                    context.current_line,
                    context.current_column,
                    token,
                    format!("Effect '{}' specified multiple times - using first", prefix),
                ));
                continue;
            }
            seen_effects.insert(prefix.clone());

            apply_effect_token(prefix, value_str, &mut effects, &mut transition_seconds, &mut clear_effects);
        }
    }

    CellAction::TriggerNote {
        pitch,
        frequency_hz,
        instrument_id,
        instrument_parameters,
        effects,
        transition_seconds,
        clear_effects,
    }
}

/// Parses a pitchless instrument trigger like "noise a:0.5"
fn parse_pitchless_trigger(tokens: &[&str], context: &mut ParserContext) -> CellAction {
    let instrument_id = find_instrument_by_name(tokens[0]).unwrap_or(4); // Default to noise
    let (effects, transition_seconds, clear_effects) = parse_effect_tokens(&tokens[1..], context);

    CellAction::TriggerPitchless {
        instrument_id,
        instrument_parameters: Vec::new(),
        effects,
        transition_seconds,
        clear_effects,
    }
}

/// Parses effect-only changes like "a:0.5 p:-0.3"
fn parse_effect_change(tokens: &[&str], context: &mut ParserContext) -> CellAction {
    let (effects, transition_seconds, clear_first) = parse_effect_tokens(tokens, context);

    CellAction::ChangeEffects {
        effects,
        transition_seconds,
        clear_first,
    }
}

/// Parses master bus effects
fn parse_master_effects(tokens: &[&str], context: &mut ParserContext) -> CellAction {
    // Determine starting index (skip "master" if present)
    let start_index = if find_instrument_by_name(tokens[0]).is_some() { 1 } else { 0 };
    let effect_tokens = &tokens[start_index..];

    let mut should_clear = false;
    let mut transition_seconds = 0.0;
    let mut master_effects: Vec<(String, Vec<f32>)> = Vec::new();
    let mut seen_effects: HashSet<String> = HashSet::new();

    // First pass: check for clear
    for token in effect_tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear"
            || token_lower == "cl"
            || token_lower.starts_with("clear:")
            || token_lower.starts_with("cl:")
        {
            should_clear = true;

            // Extract transition time from clear:X
            if token_lower.starts_with("clear:") || token_lower.starts_with("cl:") {
                if let Some(colon_pos) = token.find(':') {
                    let params = parse_parameter_list(&token[colon_pos + 1..]);
                    if !params.is_empty() {
                        transition_seconds = params[0].max(0.0);
                    }
                }
            }
        }
    }

    // Second pass: parse effects
    for token in effect_tokens {
        let token_lower = token.to_lowercase();

        // Skip clear tokens
        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }
        if token_lower.starts_with("clear:") || token_lower.starts_with("cl:") {
            continue;
        }

        if let Some(colon_pos) = token.find(':') {
            let effect_name = token[..colon_pos].to_lowercase();
            let value_str = &token[colon_pos + 1..];

            // Handle transition
            if effect_name == "tr" || effect_name == "transition" {
                let params = parse_parameter_list(value_str);
                if !params.is_empty() {
                    transition_seconds = params[0].max(0.0);
                }
                continue;
            }

            // Validate it's a master effect
            match effect_name.as_str() {
                "rv" | "reverb" | "rv2" | "reverb2" | "dl" | "delay" | "a" | "amplitude" | "p" | "pan" | "ch" | "chorus" => {
                    if seen_effects.contains(&effect_name) {
                        context.errors.push(ParseError::warning(
                            context.current_line,
                            context.current_column,
                            token,
                            format!("Master effect '{}' specified multiple times", effect_name),
                        ));
                        continue;
                    }
                    seen_effects.insert(effect_name.clone());

                    let params = parse_parameter_list(value_str);
                    master_effects.push((effect_name, params));
                }
                _ => {
                    context.errors.push(ParseError::warning(
                        context.current_line,
                        context.current_column,
                        token,
                        format!(
                            "Effect '{}' cannot be applied to master bus. Use: a, p, rv, rv2, dl, ch",
                            effect_name
                        ),
                    ));
                }
            }
        }
    }

    CellAction::MasterEffects {
        clear_first: should_clear,
        transition_seconds,
        effects: master_effects,
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Parses effect tokens and returns (effects, transition_seconds, clear_first)
fn parse_effect_tokens(
    tokens: &[&str],
    context: &mut ParserContext,
) -> (ChannelEffectState, f32, bool) {
    let mut effects = ChannelEffectState::default();
    effects.initialize_chorus_buffer(48000);
    let mut transition_seconds = 0.0;
    let mut clear_first = false;
    let mut seen_effects: HashSet<String> = HashSet::new();

    // First pass: check for clear
    for token in tokens {
        let token_lower = token.to_lowercase();
        if token_lower == "clear" || token_lower == "cl" {
            clear_first = true;
            break;
        }
    }

    // Second pass: parse effects
    for token in tokens {
        let token_lower = token.to_lowercase();

        if token_lower == "clear" || token_lower == "cl" {
            continue;
        }

        if let Some(colon_pos) = token.find(':') {
            let effect_name = token[..colon_pos].to_lowercase();
            let value_str = &token[colon_pos + 1..];

            if seen_effects.contains(&effect_name) {
                context.errors.push(ParseError::warning(
                    context.current_line,
                    context.current_column,
                    token,
                    format!("Effect '{}' specified multiple times", effect_name),
                ));
                continue;
            }
            seen_effects.insert(effect_name.clone());

            apply_effect_token(&effect_name, value_str, &mut effects, &mut transition_seconds, &mut clear_first);
        }
    }

    (effects, transition_seconds, clear_first)
}

/// Applies an effect token to an effect state
fn apply_effect_token(
    effect_name: &str,
    value_str: &str,
    effects: &mut ChannelEffectState,
    transition_seconds: &mut f32,
    clear_effects: &mut bool,
) {
    let params = parse_parameter_list(value_str);

    match effect_name {
        "a" | "amplitude" => {
            if !params.is_empty() {
                effects.amplitude = params[0].clamp(0.0, 1.0);
            }
        }
        "p" | "pan" => {
            if !params.is_empty() {
                effects.pan = params[0].clamp(-1.0, 1.0);
            }
        }
        "v" | "vibrato" => {
            if params.len() >= 2 {
                effects.vibrato_rate_hz = params[0].max(0.0);
                effects.vibrato_depth_semitones = params[1].max(0.0);
            }
        }
        "t" | "tremolo" => {
            if params.len() >= 2 {
                effects.tremolo_rate_hz = params[0].max(0.0);
                effects.tremolo_depth = params[1].clamp(0.0, 1.0);
            }
        }
        "b" | "bitcrush" => {
            if !params.is_empty() {
                effects.bitcrush_bits = (params[0] as u8).clamp(1, 16);
            }
        }
        "d" | "distortion" => {
            if !params.is_empty() {
                effects.distortion_amount = params[0].clamp(0.0, 1.0);
            }
        }
        "ch" | "chorus" => {
            if !params.is_empty() {
                effects.chorus_mix = params[0].clamp(0.0, 1.0);
            }
            if params.len() > 1 {
                effects.chorus_rate_hz = params[1].clamp(0.1, 5.0);
            }
            if params.len() > 2 {
                effects.chorus_depth_ms = params[2].clamp(0.5, 10.0);
            }
            if params.len() > 3 {
                effects.chorus_feedback = params[3].clamp(0.0, 0.9);
            }
        }
        "tr" | "transition" => {
            if !params.is_empty() {
                *transition_seconds = params[0].max(0.0);
            }
        }
        "cl" | "clear" => {
            *clear_effects = true;
            if !params.is_empty() {
                *transition_seconds = params[0].max(0.0);
            }
        }
        _ => {
            // Unknown effect - ignore (error already reported if needed)
        }
    }
}

/// Parses a parameter list like "0.5'0.3" into [0.5, 0.3]
fn parse_parameter_list(params_str: &str) -> Vec<f32> {
    params_str
        .split('\'')
        .filter_map(|s| s.parse::<f32>().ok())
        .collect()
}

/// Checks if an effect name is a master-only effect
fn is_master_effect(token: &str) -> bool {
    let token_lower = token.to_lowercase();

    // Check for effects that are master-only when they appear first
    if let Some(colon_pos) = token.find(':') {
        let effect_name = &token_lower[..colon_pos];
        matches!(effect_name, "rv" | "reverb" | "rv2" | "reverb2" | "dl" | "delay")
    } else {
        false
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_comments() {
        assert_eq!(strip_comments("test // comment"), "test ");
        assert_eq!(strip_comments("c#4 sine // sharp note"), "c#4 sine ");
        assert_eq!(strip_comments("# full comment"), "");
    }

    #[test]
    fn test_parse_parameter_list() {
        assert_eq!(parse_parameter_list("0.5"), vec![0.5]);
        assert_eq!(parse_parameter_list("0.5'0.3"), vec![0.5, 0.3]);
        assert_eq!(parse_parameter_list("1'2'3"), vec![1.0, 2.0, 3.0]);
    }
}

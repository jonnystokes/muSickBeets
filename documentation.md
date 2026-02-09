# muSickBeets Documentation

## What is muSickBeets?

**muSickBeets** is a CSV-driven music tracker synthesizer written in Rust. It allows you to compose music by writing simple text files - no complex DAW required. Think of it as programming music: each row is a moment in time, each column is a voice/channel.

### Key Features

- **12 independent channels** - Play up to 12 sounds simultaneously
- **5 built-in instruments** - Sine, Trisaw, Square, Noise, Pulse
- **6 preset envelopes** - From punchy percussion to smooth pads
- **Per-channel effects** - Amplitude, pan, vibrato, tremolo, bitcrush, distortion, chorus
- **Master bus effects** - Reverb (simple & advanced), delay, chorus
- **Real-time playback** - Hear your music as it plays
- **WAV export** - Export high-quality 48kHz stereo WAV files
- **Smooth transitions** - Glide between notes and effect changes
- **Forgiving parser** - Handles sloppy input gracefully

---

## Quick Start

1. Create a CSV file with your song (see `assets/song.csv` for example)
2. Run: `cargo run --release`
3. Listen to your creation!

---

## Song File Format

### Basic Structure

```csv
Voice0,Voice1,Voice2,Voice3,Voice4,Voice5,Voice6,Voice7,Voice8,Voice9,Voice10,Voice11
config, title: My Song, export_wav: true, tick_duration: 0.25
c4 sine a:0.5,e4 sine a:0.5,g4 sine a:0.5,,,,,,,,,
-,-,-,,,,,,,,,
.,.,.,,,,,,,,
```

### Row Types

| Symbol | Meaning |
|--------|---------|
| `c4 sine a:0.5` | Trigger note C4 with sine wave, amplitude 0.5 |
| `-` | Sustain (keep playing current note) |
| `.` | Release (begin fade out) |
| (empty) | No change |
| `//` or `#` | Comment (entire line) |
| `config` | Configuration row (must be row 2) |
| `master` | Master bus effects |

### Configuration Row

Place on row 2 (after header):

```csv
config, title: Song Name, export_wav: true, tick_duration: 0.25, tempo_bpm: 120
```

| Setting | Description | Default |
|---------|-------------|---------|
| `title` | Song title | "Untitled" |
| `export_wav` | Auto-export WAV file | false |
| `tick_duration` | Seconds per row | 0.25 |
| `tempo_bpm` | Beats per minute (informational) | 120 |

---

## Instruments

### Available Instruments

| ID | Name | Aliases | Parameters | Description |
|----|------|---------|------------|-------------|
| 1 | `sine` | `sin` | none | Pure sine wave - clean, mellow |
| 2 | `trisaw` | `tri`, `saw`, `triangle`, `sawtooth` | shape: 0.0-1.0 | Morphs from triangle (0) to sawtooth (1) |
| 3 | `square` | `sq` | none | Hollow, retro 8-bit sound |
| 4 | `noise` | `white`, `whitenoise` | none | White noise - no pitch required |
| 5 | `pulse` | `pwm` | width: 0.0-1.0 | Variable pulse width (0.5 = square) |

### Usage Examples

```csv
// Basic note trigger
c4 sine

// With amplitude
c4 sine a:0.5

// Trisaw with shape parameter (0.0=triangle, 1.0=sawtooth)
c4 trisaw:0.5 a:0.6

// Pulse with width (0.5=square, lower=thinner)
c4 pulse:0.25 a:0.4

// Noise (no pitch needed)
noise a:0.5
```

### Instrument Parameter Ranges

| Instrument | Parameter | Range | Default | Description |
|------------|-----------|-------|---------|-------------|
| trisaw | shape | 0.0 - 1.0 | 0.5 | 0=triangle, 1=sawtooth |
| pulse | width | 0.0 - 1.0 | 0.5 | Pulse width (duty cycle) |

---

## Channel Effects

Effects applied to individual channels. Use `effect:value` syntax.

### Effect Reference

| Effect | Aliases | Parameters | Range | Description |
|--------|---------|------------|-------|-------------|
| `a` | `amplitude` | level | 0.0 - 1.0 | Volume control |
| `p` | `pan` | position | -1.0 - 1.0 | Stereo position (-1=left, 0=center, 1=right) |
| `v` | `vibrato` | rate, depth | rate: 0-20 Hz, depth: 0-2 semitones | Pitch wobble |
| `t` | `tremolo` | rate, depth | rate: 0-20 Hz, depth: 0.0-1.0 | Volume wobble |
| `b` | `bitcrush` | bits | 1 - 16 | Bit depth reduction (lower = crunchier) |
| `d` | `distortion` | amount | 0.0 - 1.0 | Overdrive/saturation |
| `ch` | `chorus` | mix, rate, depth, feedback | see below | Adds width and richness |
| `tr` | `transition` | seconds | 0.0 - 5.0 | Smooth transition time |
| `cl` | `clear` | seconds | 0.0 - 5.0 | Reset effects to default |

### Chorus Parameters

```csv
ch:mix'rate'depth'feedback
```

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| mix | 0.0 - 1.0 | 0.5 | Wet/dry mix |
| rate | 0.1 - 5.0 Hz | 1.0 | LFO speed |
| depth | 0.5 - 10.0 ms | 3.0 | Modulation depth |
| feedback | 0.0 - 0.9 | 0.0 | Feedback amount |

### Usage Examples

```csv
// Volume at 50%
c4 sine a:0.5

// Pan hard left
c4 sine a:0.5 p:-1.0

// Vibrato: 5 Hz rate, 0.5 semitones depth
c4 sine a:0.6 v:5'0.5

// Tremolo: 6 Hz rate, 40% depth
c4 sine a:0.7 t:6'0.4

// Bitcrush to 4 bits
c4 square a:0.4 b:4

// Light distortion
c4 sine a:0.5 d:0.3

// Rich chorus
c4 trisaw:0.5 a:0.5 ch:0.5'1.5'3.0'0.3

// Smooth transition over 0.5 seconds
e4 sine a:0.5 transition:0.5

// Multiple effects combined
c4 sine a:0.6 p:-0.3 v:4'0.2 d:0.2 ch:0.3'1.0'2.0'0.1
```

---

## Master Bus Effects

Effects applied to the entire mix. Place in Voice0 column with `master` prefix.

### Master Effect Reference

| Effect | Aliases | Parameters | Description |
|--------|---------|------------|-------------|
| `rv` | `reverb` | room, mix | Simple reverb |
| `rv2` | `reverb2` | room, decay, damping, mix, predelay | Advanced algorithmic reverb |
| `dl` | `delay` | time, feedback | Echo/delay effect |
| `ch` | `chorus` | mix, rate, depth, spread | Stereo chorus |
| `a` | `amplitude` | level | Master volume |
| `p` | `pan` | position | Master stereo position |
| `clear` | `cl` | seconds | Reset all master effects |

### Reverb Parameters

**Simple Reverb (rv)**
```csv
master rv:room'mix
```

| Parameter | Range | Description |
|-----------|-------|-------------|
| room | 0.0 - 1.0 | Room size |
| mix | 0.0 - 1.0 | Wet/dry mix |

**Advanced Reverb (rv2)**
```csv
master rv2:room'decay'damping'mix'predelay
```

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| room | 0.0 - 1.0 | 0.5 | Room size |
| decay | 0.1 - 10.0 | 2.0 | Decay time in seconds |
| damping | 0.0 - 1.0 | 0.5 | High frequency damping |
| mix | 0.0 - 1.0 | 0.3 | Wet/dry mix |
| predelay | 0.0 - 100.0 | 20.0 | Pre-delay in milliseconds |

### Delay Parameters

```csv
master dl:time'feedback
```

| Parameter | Range | Description |
|-----------|-------|-------------|
| time | 0.01 - 2.0 | Delay time in seconds |
| feedback | 0.0 - 0.95 | Feedback amount (echo repeats) |

### Usage Examples

```csv
// Simple reverb
master rv:0.6'0.4

// Rich reverb with long decay
master rv2:0.7'3.0'0.4'0.5'25.0

// Quarter-note delay with 50% feedback
master dl:0.25'0.5

// Combine reverb and delay
master rv2:0.5'2.0'0.3'0.35'20.0 dl:0.3'0.4

// Clear all master effects
master clear
```

---

## Envelopes

Envelopes shape how notes start and stop. They're defined per-instrument but control the volume over time.

### Preset Envelopes

| ID | Name | Attack | Decay | Sustain | Release | Best For |
|----|------|--------|-------|---------|---------|----------|
| 0 | `default` | 10ms | 100ms | 85% | 2.0s | General purpose |
| 1 | `pluck` | 5ms | 300ms | 30% | 0.5s | Plucked strings, staccato |
| 2 | `pad` | 500ms | 200ms | 90% | 3.0s | Ambient pads, strings |
| 3 | `percussion` | 1ms | 0ms | 100% | 100ms | Drums, hits |
| 4 | `organ` | 5ms | 0ms | 100% | 50ms | Sustained organ tones |
| 5 | `swell` | 2.0s | 0ms | 100% | 2.0s | Dramatic swells |

### Envelope Curve Types

- **Linear** - Straight line change
- **Exponential** - Natural decay curve (faster start, slower end)
- **Logarithmic** - Punchy curve (slower start, faster end)

### Simulating Envelope Variations

You can simulate different envelope behaviors using:

**Fast attack (percussion-like):**
```csv
c4 sine a:0.8
.            // Release immediately
```

**Slow attack (pad-like):**
```csv
c4 sine a:0.1
a:0.3        // Fade in
a:0.5
a:0.7
a:0.8
-            // Hold at peak
```

**Using transitions for smooth changes:**
```csv
c4 sine a:0.5 transition:0.5    // Smooth 0.5s attack
-
e4 sine a:0.5 transition:0.3    // Glide to next note
```

---

## Creating Sounds by Combining Elements

### Layering Techniques

**Thick bass (two channels, slight detune feel):**
```csv
c2 sine a:0.4,c2 trisaw:0.7 a:0.3,,,
```

**Rich pad (multiple waveforms + effects):**
```csv
c4 trisaw:0.3 a:0.3 ch:0.4'1.0'3.0'0.2,c4 sine a:0.25 v:3'0.2,c4 pulse:0.4 a:0.2,,
```

**Punchy lead:**
```csv
c5 square a:0.5 d:0.2,c5 sine a:0.3,,,,
```

### Effect Combinations

**Wobbly synth:**
```csv
c4 trisaw:0.5 a:0.5 v:6'0.5 t:4'0.3
```

**Lo-fi crunch:**
```csv
c4 square a:0.4 b:6 d:0.4
```

**Ethereal pad:**
```csv
master rv2:0.7'3.0'0.3'0.45'30.0
c3 sine a:0.4 ch:0.5'0.8'4.0'0.3 v:2'0.1
```

**Rhythmic delay:**
```csv
master dl:0.125'0.6
c4 pulse:0.3 a:0.5
.
-
-
// Delay creates rhythm
```

---

## Adding Custom Components

### File Structure

```
src/
├── main.rs          // Configuration, entry point
├── instruments.rs   // Instrument definitions
├── effects/
│   └── mod.rs       // Effect processing
├── envelope.rs      // Envelope definitions
├── parser.rs        // CSV parsing
├── channel.rs       // Channel state
├── master_bus.rs    // Master effects
├── engine.rs        // Playback engine
├── audio.rs         // WAV export
└── helper.rs        // Utilities
```

### Adding a New Instrument

**Step 1: Edit `src/instruments.rs`**

Add to `INSTRUMENT_REGISTRY` array (around line 115):

```rust
// Add after the last InstrumentDefinition
InstrumentDefinition {
    id: 6,  // Next available ID
    name: "myinstrument",
    aliases: &["myinst", "mi"],
    requires_pitch: true,  // false for noise-like instruments
    is_playable: true,
    default_attack_seconds: 0.01,
    default_release_seconds: 0.3,
    parameters: &[
        InstrumentParameter {
            name: "myparam",
            default_value: 0.5,
            min_value: 0.0,
            max_value: 1.0,
            description: "My custom parameter",
        },
    ],
    description: "My custom instrument",
},
```

**Step 2: Add the sample generation function**

Add after existing generate functions (around line 450):

```rust
/// Generates my custom instrument
fn generate_myinstrument(phase: f32, params: &[f32], rng: &mut RandomNumberGenerator) -> f32 {
    let myparam = params.get(0).copied().unwrap_or(0.5);

    // Your waveform generation code here
    // phase goes from 0 to 2*PI
    // Return a value between -1.0 and 1.0

    (phase * myparam).sin()  // Example
}
```

**Step 3: Add to the generate_sample match**

In `generate_sample` function (around line 500):

```rust
match instrument_id {
    // ... existing cases ...
    6 => generate_myinstrument(phase, params, rng),
    _ => 0.0,
}
```

### Adding a New Channel Effect

**Step 1: Edit `src/effects/mod.rs`**

Add field to `ChannelEffectState` (around line 150):

```rust
pub struct ChannelEffectState {
    // ... existing fields ...

    // My new effect
    pub myeffect_amount: f32,
    pub myeffect_rate: f32,
}
```

Update `Default` impl:

```rust
impl Default for ChannelEffectState {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            myeffect_amount: 0.0,
            myeffect_rate: 1.0,
        }
    }
}
```

**Step 2: Add processing in `apply_channel_effects`**

In `apply_channel_effects` function (around line 376):

```rust
pub fn apply_channel_effects(...) -> (f32, f32) {
    let mut sample = input_sample;

    // ... existing effects ...

    // My effect
    if effects.myeffect_amount > 0.0 {
        // Your effect processing code
        sample = sample * (1.0 - effects.myeffect_amount * 0.5);
    }

    // ... rest of function ...
}
```

**Step 3: Add parsing in `src/parser.rs`**

In `apply_effect_token` function (around line 1017):

```rust
match effect_name {
    // ... existing cases ...

    "my" | "myeffect" => {
        if !params.is_empty() {
            effects.myeffect_amount = params[0].clamp(0.0, 1.0);
        }
        if params.len() > 1 {
            effects.myeffect_rate = params[1].clamp(0.1, 10.0);
        }
    }

    _ => { /* unknown */ }
}
```

### Adding a New Master Effect

**Step 1: Edit `src/effects/mod.rs`**

Add to `MasterEffectState`:

```rust
pub struct MasterEffectState {
    // ... existing fields ...

    pub mymaster_enabled: bool,
    pub mymaster_amount: f32,
    // Add buffers if needed
    pub mymaster_buffer: Vec<f32>,
}
```

**Step 2: Add processing in `apply_master_effects`**

```rust
pub fn apply_master_effects(...) -> (f32, f32) {
    // ... existing effects ...

    // My master effect
    if effects.mymaster_enabled {
        // Process left and right channels
        left = process_mymaster(left, effects);
        right = process_mymaster(right, effects);
    }

    // ... rest of function ...
}
```

**Step 3: Add parsing in `src/master_bus.rs`**

In `apply_effect` method (around line 328):

```rust
match effect_name.to_lowercase().as_str() {
    // ... existing cases ...

    "mym" | "mymaster" => {
        if !parameters.is_empty() {
            let amount = parameters[0].clamp(0.0, 1.0);
            self.apply_with_transition(|target| {
                target.mymaster_amount = amount;
                target.mymaster_enabled = amount > 0.0;
            }, transition_seconds);
        }
    }

    _ => { /* unknown */ }
}
```

### Adding a New Envelope

**Edit `src/envelope.rs`**

Add to `ENVELOPE_REGISTRY` array (around line 165):

```rust
EnvelopeDefinition {
    id: 6,  // Next available ID
    name: "myenvelope",
    description: "My custom envelope shape",
    attack_time_seconds: 0.05,
    decay_time_seconds: 0.2,
    sustain_level: 0.7,
    release_time_seconds: 1.0,
    attack_curve: EnvelopeCurveType::Logarithmic,
    attack_curve_strength: 2.0,
    decay_curve: EnvelopeCurveType::Exponential,
    decay_curve_strength: 2.0,
    release_curve: EnvelopeCurveType::Exponential,
    release_curve_strength: 2.5,
},
```

---

## Configuration Constants

Edit `src/main.rs` to change global settings:

```rust
// Audio settings
const SAMPLE_RATE: u32 = 48000;           // Sample rate in Hz
const CHANNEL_COUNT: usize = 12;          // Number of voices
const TICK_DURATION_SECONDS: f32 = 0.25;  // Seconds per row

// Buffer settings (for heavy effects)
const AUDIO_BUFFER_SIZE: u32 = 4096;      // Samples per callback
const AUDIO_BUFFER_COUNT: u32 = 3;        // Number of buffers
const MAX_EFFECT_BUFFER_SECONDS: f32 = 4.0;
const MAX_MODULATION_DELAY_MS: f32 = 100.0;

// Envelope defaults
const DEFAULT_ATTACK_SECONDS: f32 = 0.01;
const DEFAULT_RELEASE_SECONDS: f32 = 0.5;
```

---

## Troubleshooting

### Audio Glitches/Crackling

- Increase `AUDIO_BUFFER_SIZE` in `src/main.rs`
- Reduce effect complexity
- Use `--release` build: `cargo run --release`

### Notes Cut Off Too Fast

- Use `-` to sustain notes
- Check release time in envelope
- Use `transition` for smooth changes

### Effects Not Working

- Check effect syntax: `effect:value` (colon, not equals)
- Parameters separated by `'` (apostrophe): `ch:0.5'1.0'3.0`
- Master effects need `master` prefix

### Parser Warnings

- The parser is forgiving and will warn about issues
- Check the console output for helpful messages

---

## Example Songs

See `assets/` directory:
- `song.csv` - Creative musical piece
- `test_demo.csv` - Comprehensive feature demonstration

---

## Building

```bash
# Debug build
cargo build

# Release build (faster, recommended)
cargo build --release

# Run
cargo run --release

# Run with specific song file
cargo run --release -- assets/mysong.csv

# Run tests
cargo test
```

---

## License

MIT License - See LICENSE file for details.

---

*muSickBeets - Making music with spreadsheets since 2024*

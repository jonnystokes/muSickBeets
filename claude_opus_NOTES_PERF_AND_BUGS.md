# muSickBeets FFT Analyzer: Deep Code Review

**Reviewer:** Claude Opus 4 (claude-opus-4-6)
**Date:** 2026-02-24
**Scope:** Read-only analysis of `src/fft_analyzer/` — bugs, edge cases, design issues, performance

---

## PART 1: BUGS AND LOGIC ERRORS

### BUG-1: WAV file loading blocks the main thread (callbacks_file.rs:90)

`AudioData::from_wav_file()` is called **synchronously on the main GUI thread** inside the Open button callback. For large WAV files (hundreds of MB), this reads the entire file into memory, decodes all samples, and downmixes stereo to mono — all while the FLTK event loop is frozen. The status bar says "Loading audio..." but will never actually display that message because `app::awake()` is called *after* setting the status bar value but the file read happens *before* the event loop can process the awake.

The FFT is correctly spawned on a background thread, but the file I/O that precedes it is not. This is the primary cause of the "file open dialog freeze" — the freeze happens not from the dialog itself but from the synchronous WAV decode immediately after.

**Severity:** High (UI freeze for large files)

### BUG-2: `AudioData::from_wav_file` normalizes source audio destructively (callbacks_file.rs:96-103)

The raw WAV data is peak-normalized *before* being stored as `st.audio_data`. This means the original signal amplitudes are permanently altered. If the user later changes normalization settings or expects the spectrogram magnitudes to reflect the actual WAV file's dB levels, the values will be wrong. The spectrogram's magnitude data will reflect the normalized audio, not the original.

Separately, if `normalize_peak` is set to 0.97 and the audio already peaks at exactly 0.97, the check `(peak - target_peak).abs() < 0.01` will skip normalization (correct), but if it peaks at 0.96, it will apply a gain of 1.01x, which is functionally meaningless but wastes a pass over all samples.

**Severity:** Medium (misleading dB values in spectrogram readout)

### BUG-3: Audio device sample rate mismatch on subsequent loads (audio_player.rs:55-57)

The audio device is initialized once with `init_device(audio.sample_rate)` and never recreated. If the user loads a 44100 Hz WAV, then loads a 48000 Hz WAV, the miniaudio device is still configured for 44100 Hz. The 48000 Hz audio will play at the wrong speed and pitch (slower, lower-pitched).

The guard `if self.device.is_none()` means the device is only created once. After that, all subsequent loads reuse the same device regardless of sample rate changes.

**Severity:** High (incorrect playback for files with different sample rates in a session)

### BUG-4: Reconstruction magnitude scaling mismatch (fft_engine.rs:80-81 vs reconstructor.rs:82-88)

In the forward FFT (`fft_engine.rs`), magnitudes are scaled:
```rust
let amplitude_scale = if bin_idx == 0 || bin_idx == num_bins - 1 { 1.0 } else { 2.0 };
magnitudes.push((complex_val.norm() / n_fft as f32) * amplitude_scale);
```

In reconstruction (`reconstructor.rs`), the stored magnitudes are used directly to build the IFFT spectrum:
```rust
spectrum[i] = Complex::from_polar(mag, phase);
```

But the forward pass divided by `n_fft` and multiplied by 2 for non-DC/Nyquist bins. The IFFT output is then divided by `n_fft` again (line 96-98). This means the reconstruction applies `1/n_fft` twice — once during forward magnitude extraction and once during IFFT normalization. The x2 scaling for non-DC bins partially compensates, but the overall gain is still wrong.

The overlap-add window normalization at the end partially masks this because it divides by the squared window sum, which adjusts the amplitude. But the mathematical correctness of the round-trip is questionable: the magnitudes stored in the spectrogram are display-oriented (with the 2x factor for one-sided spectrum), not reconstruction-oriented.

**Severity:** Medium (audio reconstruction has incorrect amplitude; masked by post-normalization)

### BUG-5: DC and Nyquist bin phase handling in reconstruction (reconstructor.rs:85-86)

```rust
if i == 0 || i == spectrum.len() - 1 {
    spectrum[i] = Complex::new(mag * phase.cos(), 0.0);
}
```

For the DC bin (i==0) and Nyquist bin (i==spectrum.len()-1), the imaginary part is forced to zero. This is correct for DC (always real), but for the Nyquist bin it discards the sign information. The Nyquist bin should be real-valued, but the sign matters. Using `mag * phase.cos()` when the phase came from `complex_val.arg()` should be correct since `cos(arg)` recovers the sign, but this is fragile — if the forward FFT stored phase as `atan2(im, re)` and im was exactly 0, the phase could be 0 or PI, and `cos(0) = 1` vs `cos(PI) = -1` gives the correct sign. So this is technically correct but relies on floating-point precision.

**Severity:** Low (functionally correct in practice)

### BUG-6: `format_with_commas` breaks for negative numbers (callbacks_draw.rs:571-584)

```rust
fn format_with_commas(n: i64) -> String {
    let s = n.to_string();
    // ...
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }
    result
}
```

If `n` is negative (e.g. `-1000`), the string is `"-1000"` with length 5. At `i=1` (the `1`), `(5-1) % 3 == 1 != 0`, no comma. At `i=2` (the `0`), `(5-2) % 3 == 0`, comma inserted: `"-1,000"`. Wait — actually this is `"-1,000"` which is correct. But at `i=3`, `(5-3) % 3 == 2 != 0`. So actually the output is `-1,000` which is correct. False alarm on this specific case, but for `-1000000` the digits length is 8: positions would be `-`, `1`, `0`, `0`, `0`, `0`, `0`, `0`. At i=2, (8-2)%3 = 0 → comma. Result: `-1,0,00,000`. That's wrong. The minus sign is counted as a character but shouldn't be in the grouping logic.

**Severity:** Low (frequency labels are always positive integers in practice, but the function is generically named)

### BUG-7: `Spectrogram::from_frames` assumes sorted input (spectrogram.rs:28-29)

```rust
let min_time = frames.first().unwrap().time_seconds;
let max_time = frames.last().unwrap().time_seconds;
```

This assumes frames are sorted by time. If frames arrive out of order (possible from `par_iter` if the collect doesn't preserve order — but rayon's `par_iter().map().collect()` does preserve order), this would be wrong. In practice, rayon's `into_par_iter` on a range preserves order in the collected Vec, so this is safe. However, `from_frames` is also called in CSV import where frames come from a BTreeMap (sorted by string key), so frame order should also be correct there. But the function has no defensive sort.

**Severity:** Low (safe in current usage, but fragile API contract)

### BUG-8: CSV time key collision due to f64 formatting precision (csv_export.rs:67, csv_export.rs:210-219)

Export formats time as `{:.5}` (5 decimal places). Import groups rows by the exact time string. If two frames have times that differ by less than 0.000005 seconds (0.005 ms), they'll be formatted identically and merged into a single frame during import. At 48kHz with hop=1 sample, frame spacing is ~0.00002s — that would survive. But this is a data fidelity issue for very dense spectrograms.

Also, the import uses `String` keys in a `BTreeMap`, so `"0.01000"` and `"0.0100"` would be different keys even if they represent the same time. The consistent `{:.5}` format prevents this in round-trip, but manual CSV editing could trigger it.

**Severity:** Low (unlikely at normal FFT parameters)

### BUG-9: `view_hash` is a weak hash with collision risk (spectrogram_renderer.rs:42-72)

The hash function uses `wrapping_mul(31)` with `as u64` casts of floating-point values. Negative floats cast to u64 will wrap (saturate to 0 on some platforms), and the multiplication chain is weak. Two different view states could easily produce the same hash. If a collision occurs, the spectrogram won't redraw when it should — the user would see stale data until something else triggers a redraw.

The `cache_valid` flag provides a separate invalidation path, so the hash is only checked when `cache_valid == true`. Most view changes go through `invalidate()` which sets `cache_valid = false`, bypassing the hash entirely. The hash is mainly a secondary guard. But subtle changes (e.g., tiny scrollbar movements) that don't call `invalidate()` could hit collisions.

**Severity:** Low-Medium (could cause stale renders in edge cases)

### BUG-10: `play()` does not auto-recompute after dirty change (callbacks_ui.rs:573-579)

```rust
if st.dirty {
    drop(st);
    btn_rerun.do_callback();
    return;
}
st.audio_player.play();
```

When Play is pressed and `dirty == true`, it triggers recompute and returns — it never starts playback. After the recompute finishes (asynchronously), playback doesn't auto-start. The user has to press Play a second time. This is arguably a UX issue rather than a bug, but the intent seems to be "recompute then play."

**Severity:** Low (UX annoyance, not data corruption)

---

## PART 2: EDGE CASES AND UNSAFE ASSUMPTIONS

### EDGE-1: Division by zero guards using `.max(1)` on sample_rate

Throughout the codebase, `sample_rate.max(1)` is used to avoid division by zero. This is defensive, but if sample_rate is somehow 0 (e.g., from a corrupted WAV file or bad CSV import), the resulting calculations would use sample_rate=1, producing nonsensical time values (a 48000-sample file would appear to be 48000 seconds long). There's no early rejection of sample_rate==0 at the WAV loading boundary.

### EDGE-2: Window length of 2 is mathematically valid but practically useless

The minimum window length is clamped to 2 (segmentation_solver.rs). An FFT of size 2 produces 2 frequency bins (DC + Nyquist) — no useful spectral information. The UI allows this without warning.

### EDGE-3: Zero-padded FFT size can be enormous

With window_length=65536 and zero_pad_factor=8, `n_fft_padded()` returns 524288. Each frame allocates a 524288-element f32 vector for the FFT. With rayon parallelizing across N CPU cores, peak memory usage is `N * 524288 * 4 bytes * 2 (input + output) = N * 4 MB`. On a 16-core machine, that's 64 MB just for active FFT buffers, plus the planner overhead. This is manageable but could surprise users.

### EDGE-4: `view.freq_min_hz` can be set below 1 Hz

`reset_zoom()` sets `freq_min_hz = 0.0`, but log-scale calculations require `freq_min_hz > 0`. The `y_to_freq` and `freq_to_y` methods internally clamp to `min(1.0)`, so 0 Hz is mapped to 1 Hz. But the viewport state stores 0.0, creating a inconsistency between the stored value and what's displayed.

### EDGE-5: Spectrogram mouse readout binary search edge case (spectrogram.rs:76)

```rust
let idx = self.frames
    .binary_search_by(|f| f.time_seconds.partial_cmp(&time_seconds).unwrap())
    .unwrap_or_else(|i| i.min(self.frames.len() - 1));
```

`partial_cmp` on f64 can return `None` for NaN. The `.unwrap()` would panic if either `f.time_seconds` or `time_seconds` is NaN. NaN could arise from division of 0.0/0.0 in time calculations if data bounds are degenerate.

### EDGE-6: `reconstructed_audio.take()` in waveform draw (callbacks_draw.rs:259)

```rust
let audio_opt = st.reconstructed_audio.take();
// ... use audio_opt ...
st.reconstructed_audio = audio_opt;
```

This temporarily removes the reconstructed audio from AppState to avoid double-borrow issues. If the draw callback panicked between `take()` and the put-back, the reconstructed audio would be permanently lost. Rust unwinding would propagate through FLTK's C code, which is undefined behavior anyway, so this is academic — but it's a code smell.

---

## PART 3: PERFORMANCE ANALYSIS

### PERF-1: CRITICAL — Spectrogram rendering blocks the GUI thread

`SpectrogramRenderer::rebuild_cache()` (spectrogram_renderer.rs:126) is called from within the spectrogram's `draw()` callback, which runs on the main GUI thread. For a spectrogram of 1000 frames x 4097 bins rendered to a 1200x800 pixel area:

- **Active bins computation** (line 149): `par_iter` over 1000 frames, each sorting up to 4097 bins — O(n log n) per frame. Total: ~1000 * 4097 * 12 = ~49M operations.
- **Row bin lookup** (line 173): 800 iterations with a **linear scan** of frequencies (`for (i, &f) in first_frame.frequencies.iter().enumerate()`). With 4097 bins, that's 800 * 4097 = 3.3M iterations. This should be a binary search.
- **Pixel rendering** (line 212): `par_chunks_mut` over 800 rows, each processing 1200 pixels. That's 960K pixels, each requiring a frame lookup and LUT lookup — fast per pixel, but the total is significant.

On a Mesa llvmpipe (software rendering) system, the `image.draw()` call that blits the RGB buffer to screen is also CPU-bound. FLTK's software rendering path copies the image pixel-by-pixel.

**Combined effect:** Every zoom, scroll, or parameter change that invalidates the cache causes a full rebuild during the next draw cycle. During the rebuild (which can take 50-200ms for large spectrograms on software rendering), the UI is completely frozen.

### PERF-2: HIGH — Nearest-bin lookup is O(n) per row (spectrogram_renderer.rs:181-189)

```rust
for (i, &f) in first_frame.frequencies.iter().enumerate() {
    let dist = (f - freq).abs();
    if dist < best_dist { ... }
}
```

This linear scan runs once per pixel row. Since frequencies are sorted (they come from FFT bin indices), a binary search would reduce each lookup from O(4097) to O(12). For 800 rows, this saves ~3.2M comparisons.

### PERF-3: HIGH — Active bins computation sorts per frame (spectrogram_renderer.rs:162)

```rust
bin_mags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
```

For 1000 frames, each frame sorts up to 4097 magnitude-index pairs. This is O(1000 * 4097 * log(4097)) = ~49M comparisons. If `recon_freq_count >= num_bins` (which is the default — all bins kept), the sort is completely unnecessary because all bins will be marked active. A fast-path check could skip the sort entirely.

### PERF-4: HIGH — Frame cloning for reconstruction (main_fft.rs:492-499)

```rust
let filtered_frames: Vec<_> = spec.frames
    .iter()
    .filter(|f| f.time_seconds >= proc_time_min && f.time_seconds <= proc_time_max)
    .cloned()
    .collect();
```

Each `FftFrame` contains three `Vec<f32>` (frequencies, magnitudes, phases), each with 4097 elements. Cloning N frames allocates and copies `N * 3 * 4097 * 4 = N * 49KB`. For 1000 frames, that's ~49 MB of allocation and copying on the main thread, blocking the UI during the transition from FFT to reconstruction.

### PERF-5: MEDIUM — Per-frame FFT planner allocation (fft_engine.rs:53-54)

```rust
let mut planner = RealFftPlanner::<f32>::new();
let fft = planner.plan_fft_forward(n_fft);
```

Each rayon task creates a new planner and plans a new FFT. The planner caches internally, but each thread's planner is independent, so there's no cross-thread cache sharing. For a fixed `n_fft`, all threads plan the same FFT size. A shared planner (behind a Mutex or using thread-local storage) would avoid redundant planning.

In practice, `RealFftPlanner::new()` is relatively cheap and planning is cached within each planner instance. But for short FFTs with many frames, the overhead per frame becomes a larger fraction of total work.

### PERF-6: MEDIUM — Waveform peak rendering is single-threaded (waveform_renderer.rs:170-221)

The `draw_peaks()` method iterates over pixel columns sequentially, computing min/max for each column's sample range. For wide waveform views (1000+ pixels) of long audio (millions of samples), this can iterate over many samples per column. Unlike the spectrogram renderer, the waveform renderer does not use rayon for parallelism.

### PERF-7: MEDIUM — CSV export is O(frames * bins) with per-value formatting

Each CSV row requires formatting 4 floats with specific precision (e.g., `{:.5}`, `{:.6}`). For a 1000-frame, 4097-bin spectrogram, that's ~4 million formatted writes, all synchronous on the main thread. This blocks the UI during large exports.

### PERF-8: MEDIUM — CSV import groups by string key in BTreeMap

The import creates a `BTreeMap<String, Vec<(f32, f32, f32)>>` keyed by time string. For large CSVs, this involves many string allocations and BTreeMap insertions (O(log n) per insert with string comparison). A HashMap or pre-sorted Vec approach would be faster.

### PERF-9: LOW — 16ms poll timer runs continuously

The poll timer at 16ms (60 FPS) runs all `update_info()`, scrollbar sync, and transport updates even when nothing is happening (no file loaded, no playback). The `update_info()` callback does work every tick: `derived_info()` computes segments, frequency bins, etc. When idle, this is wasted CPU.

### PERF-10: LOW — `ViewState::clone()` in draw callbacks

The spectrogram draw callback clones the entire `ViewState` (including the `Vec<GradientStop>`) every draw cycle. With 7 gradient stops, this is a small allocation, but it happens on every frame during playback (~60 FPS).

---

## PART 4: CONCURRENCY RISKS

### CONC-1: `is_processing` flag is not atomic

`is_processing` is a plain `bool` on `AppState`, accessed only via `Rc<RefCell<>>`. Since it's only ever read/written on the main thread, this is safe. But the design means there's no way for the worker thread to signal back "I'm still working" — the main thread sets it true before spawn and false after receiving the result. If the main thread somehow processes a stale message (theoretically impossible with mpsc ordering), it could clear the flag prematurely.

**Risk:** Theoretical only — safe in current design.

### CONC-2: Mutex lock in audio callback uses `unwrap()` (audio_player.rs:71)

```rust
let mut data = playback_data.lock().unwrap();
```

If the main thread panics while holding the PlaybackData mutex, the mutex becomes poisoned. The audio callback thread would then panic on `unwrap()`, which would crash the audio thread. Miniaudio might not handle this gracefully.

**Risk:** Low (panic propagation is already catastrophic)

### CONC-3: No cancellation mechanism for FFT/reconstruction workers

Once spawned, a worker thread runs to completion. If the user quickly clicks "Open" twice, the first FFT worker is still running when the second file starts loading. The `is_processing` guard prevents spawning a second worker, but the UX is: "Still processing... please wait." There's no way to cancel the first operation.

For very large files (e.g., 30-minute WAV at 48kHz = 86M samples) with large window sizes, FFT can take tens of seconds. The user is locked out during this time.

**Risk:** Medium (UX issue, not crash risk)

---

## PART 5: UI AND UX ISSUES

### UI-1: File dialog blocks the main thread (native dialog)

`NativeFileChooser` is a blocking modal dialog. On Linux with some desktop environments, the file chooser can take several seconds to initialize (especially with remote filesystems in the sidebar). During this time, the FLTK event loop is suspended and the application appears frozen. This is inherent to FLTK's native file chooser API and cannot be fixed without switching to an async file dialog or running it on a separate thread (which FLTK doesn't support for its dialogs).

### UI-2: Slider callbacks don't update state when throttled

The display sliders (threshold, brightness, gamma, ceiling, scale) use an `UpdateThrottle` that skips `spec_renderer.invalidate()` and `redraw()` when updates arrive faster than 50ms. However, the *state value is always updated*:
```rust
state.borrow_mut().view.threshold_db = val;
if throttle.borrow_mut().should_update() {
    state.borrow_mut().spec_renderer.invalidate();
    spec_display.redraw();
}
```

This means the final slider value is always stored, but the final redraw may not happen if the last slider movement falls within a throttle window. The spectrogram won't reflect the actual final value until some other action triggers a redraw.

### UI-3: Settings are relative to CWD

`settings.ini` is saved/loaded relative to the current working directory (no absolute path). If the user runs the application from different directories, they'll get different settings files, or fail to find their saved settings.

### UI-4: Color chooser returns same value on cancel

The gradient color picker (callbacks_ui.rs:942-949) uses `color_chooser_with_default()` which returns the original color on cancel. The code compares returned values to detect cancel:
```rust
if nr != cur_r || ng != cur_g || nb != cur_b { ... }
```

This means if the user selects the *exact same color* they started with, it looks like a cancel. Also if the user picks a color and then cancels, the original color is returned — this is correct. But the detection logic has a false negative when the user intentionally confirms the same color (which is harmless).

---

## PART 6: SPECIFIC PERFORMANCE SUGGESTIONS FOR SOFTWARE RENDERING

Given the constraint of Mesa llvmpipe (CPU-only rendering, no GPU):

### SUGG-1: Move spectrogram rendering off the main thread

The single biggest improvement. Instead of rendering the spectrogram image synchronously in the draw callback:

1. When the view changes, invalidate the cache and post a render request to a dedicated renderer thread (or reuse the rayon pool)
2. The draw callback shows the last valid cached image (or a "rendering..." overlay)
3. When the renderer thread finishes, it sends a message back (like the FFT/reconstruction workers) and triggers a redraw
4. This decouples rendering from the UI thread, eliminating freezes during zoom/scroll

The tradeoff is a slight visual lag (one frame behind), but the UI stays responsive.

### SUGG-2: Replace linear bin lookup with binary search in spectrogram renderer

```rust
// Current: O(n) per row
for (i, &f) in first_frame.frequencies.iter().enumerate() { ... }

// Suggested: O(log n) per row
let bin = first_frame.frequencies.partition_point(|&f| f < freq);
```

This is a simple change that would reduce row-bin mapping from ~3.3M operations to ~10K operations.

### SUGG-3: Skip active-bins sorting when all bins are kept

```rust
if freq_count >= frame.magnitudes.len() {
    // All bins active — skip sort
    vec![true; frame.magnitudes.len()]
} else {
    // Existing sort + top-N logic
}
```

The default `recon_freq_count` equals `num_frequency_bins()`, meaning all bins are kept. The sort is O(n log n) per frame for no reason.

### SUGG-4: Use `Arc<Spectrogram>` to avoid frame cloning for reconstruction

Instead of cloning filtered frames, share the spectrogram via Arc and pass bin index ranges:
- The reconstruction worker receives `Arc<Spectrogram>` plus a time range
- It accesses frames by reference, only cloning the small amount of data needed for IFFT input

This would eliminate the ~49 MB allocation+copy per reconstruction.

### SUGG-5: Move WAV file I/O to a background thread

Wrap the `AudioData::from_wav_file()` call in `std::thread::spawn`, send the result back via the existing mpsc channel (add a new `WorkerMessage::AudioLoaded` variant). This prevents the UI freeze during file loading.

### SUGG-6: Implement progressive/tiled spectrogram rendering

Instead of rendering the entire spectrogram in one pass, render it in tiles (e.g., 64-row strips). After each tile, yield control back to the event loop. This keeps the UI responsive during rendering without requiring a separate thread.

### SUGG-7: Cache FFT planner per thread using thread-local storage

```rust
thread_local! {
    static PLANNER: RefCell<RealFftPlanner<f32>> = RefCell::new(RealFftPlanner::new());
}
```

This avoids creating a new planner per rayon task. The planner internally caches FFT plans, so reusing it across tasks on the same thread saves planning overhead.

### SUGG-8: Pre-compute sample index ranges for waveform peak rendering

For the waveform's zoomed-out mode, pre-compute the sample range for each pixel column in a single pass, then optionally parallelize the min/max computation with rayon.

### SUGG-9: Recreate audio device on sample rate change

In `load_audio()`, check if the sample rate differs from the current device's rate. If so, destroy and recreate the device. This fixes BUG-3 and ensures correct playback for all files.

### SUGG-10: Idle-aware poll timer

When no file is loaded and no playback is active, increase the poll interval to 100ms or use `app::wait()` instead of a fixed 16ms timer. This reduces idle CPU usage on software-rendered systems where every wakeup has a cost.

---

## PART 7: MEMORY USAGE OBSERVATIONS

### MEM-1: Spectrogram data duplication

The full spectrogram is stored as `Arc<Spectrogram>` in `AppState.spectrogram`. Each frame contains three `Vec<f32>` (frequencies, magnitudes, phases). For 1000 frames x 4097 bins:
- Memory: `1000 * 3 * 4097 * 4 bytes = ~47 MB`

The frequencies vector is redundant — all frames have identical frequency values (determined by FFT size and sample rate). Storing frequencies once and referencing them from all frames would save ~16 MB.

### MEM-2: Cached image buffers

The spectrogram renderer and waveform renderer each maintain an RGB image buffer. For a 1200x800 spectrogram: `1200 * 800 * 3 = 2.88 MB`. The waveform at 1200x100: `360 KB`. These are modest.

### MEM-3: Audio data stored multiply

- `audio_data: Option<Arc<AudioData>>` — original (normalized) audio
- `reconstructed_audio: Option<AudioData>` — reconstruction result
- `AudioPlayer.playback_data.samples` — copy of reconstructed audio for playback

The reconstructed audio exists in two copies (AppState + AudioPlayer). For a 10-second file at 48kHz, that's `10 * 48000 * 4 * 2 = 3.84 MB` — not huge, but unnecessary duplication.

---

## SUMMARY OF PRIORITIES

| Issue | Severity | Effort to Fix | Impact |
|-------|----------|---------------|--------|
| PERF-1: Spectrogram render blocks GUI | Critical | Medium | Eliminates biggest UI freeze |
| BUG-1: WAV loading blocks GUI | High | Low | Eliminates file-open freeze |
| BUG-3: Audio device sample rate mismatch | High | Low | Correct playback for all files |
| PERF-2: Linear bin search O(n) | High | Low | 300x speedup for bin lookups |
| PERF-3: Unnecessary sort when all bins kept | High | Low | Skip ~49M ops in default case |
| PERF-4: Frame cloning for reconstruction | High | Medium | Eliminate ~49 MB allocation |
| BUG-4: Forward/inverse magnitude mismatch | Medium | Medium | Correct reconstruction amplitude |
| BUG-2: Destructive source normalization | Medium | Low | Preserve original dB values |
| CONC-3: No worker cancellation | Medium | Medium | Better UX for large files |
| UI-2: Throttled slider final redraw | Low | Low | Ensure final value displayed |
| PERF-9: Idle poll timer waste | Low | Low | Reduce idle CPU on soft render |

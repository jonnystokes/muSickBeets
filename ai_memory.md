# muSickBeets Project Memory

## Project Structure
- Two binaries: `tracker` (music composition) and `fft_analyzer` (spectrogram tool)
- Binary entry points: `src/tracker/main.rs` and `src/fft_analyzer/main_fft.rs`
- `src/fft_analyzer/mod.rs` exists for library-style module exports (tracker doesn't use it)
- Tech: Rust, FLTK (1.5.22), miniaudio, hound, rustfft/realfft, rayon, csv

## FFT Analyzer Architecture (Redesign)
- New modules: `data/`, `processing/`, `rendering/`, `playback/`, `ui/`
- State: `Rc<RefCell<AppState>>` for UI thread, `Arc<T>` for cross-thread
- Worker pattern: `mpsc::channel` for background FFT/reconstruction -> UI
- SharedCb pattern: `Rc<RefCell<Box<dyn FnMut()>>>` for shared mutable closures

## Viewport vs Processing (Option C)
- FFT processes FULL file; sidebar Start/Stop = reconstruction time range
- Viewport zoom/scroll = visual only; "Snap to View" copies bounds to sidebar
- Time outside processing: grayed out on spectrogram. Freq cutoffs: no graying
- Waveform uses recon_start_time offset for correct viewport alignment
- Playback position = recon_start_time + audio_player.get_position_seconds()
- Dirty flag tracks settings changes; Play auto-recomputes if dirty

## Key Gotchas
- FLTK widget methods require `&mut self` - closures capturing widgets must be `FnMut`, not `Fn`
- Use `Rc<RefCell<Box<dyn FnMut()>>>` and call as `(cb.borrow_mut())()`
- **Spacebar — READ CLAUDE.md**: Three-layer defense required. `handle()` alone does NOT work for buttons/choices/checkbuttons because FLTK's internal C++ handler bypasses it. The PRIMARY defense is `clear_visible_focus()` which prevents keyboard focus entirely. See CLAUDE.md for complete rules.
- `app::add_handler()` takes `fn(Event) -> bool` (no captures) — runs AFTER widgets, useless for blocking
- **clear_visible_focus()**: Required on ALL buttons, choices, checkbuttons, sliders, scrollbars to prevent spacebar from activating them. Widgets still work by mouse click.
- **handle() vs set_callback()**: These are INDEPENDENT in fltk-rs. Setting one does not overwrite the other. But calling `handle()` twice DOES overwrite the first handler.
- **setup_spacebar_guards()**: MUST be called LAST in callback setup chain. It sets `handle()` on widgets and would be overwritten by any later `handle()` call.
- `app::event_dy()` returns `MouseWheel` enum (Up/Down/Left/Right/None), not integer
- `fltk::misc::Tooltip::enable(bool)` takes a bool; `disable()` takes no args
- `RgbImage::draw()` needs `use fltk::prelude::ImageExt`
- Module paths from submodules: use `crate::data::...` not `crate::fft_analyzer::data::...` (binary root IS the crate)
- When borrowing `st.renderer.draw(&st.data, ...)`, clone data first to avoid simultaneous mut/immut borrow
- `if let Some(ref x) = st.field { st.other = ... }` won't compile — extract into locals first, then mutate
- **FLTK Scrollbar**: `slider_size()` returns `f32` (not f64). slider_size is PURELY VISUAL (thumb size) — value range is always `[min, max]`, NOT clipped by slider_size. Use max=10000 (not 1.0) to avoid quantization. Timer must NOT call `set_value()` during user drag — use generation counters (`Rc<Cell<u64>>`) to detect active dragging. Simple: `frac = value / max`, `set_value(frac * max)`.
- **CSV load**: Must trigger reconstruction after loading FFT from CSV, otherwise no audio/waveform. Set proc_time to match spectrogram's time range to avoid graying.
- Build deps: needs libxft-dev, libpango1.0-dev, libxinerama-dev, libxcursor-dev, libxfixes-dev on Linux

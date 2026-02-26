# muSickBeets Project Memory

## Project Structure
- Two binaries: `tracker` (music composition) and `fft_analyzer` (spectrogram tool)
- Binary entry points: `src/tracker/main.rs` and `src/fft_analyzer/main_fft.rs`
- `src/fft_analyzer/mod.rs` exists for library-style module exports (tracker doesn't use it)
- Tech: Rust, FLTK (1.5.22), miniaudio, hound, rustfft/realfft, rayon, csv

## FFT Analyzer Architecture (Redesign)
- New modules: `data/`, `processing/`, `rendering/`, `playback/`, `ui/`
- State: `Rc<RefCell<AppState>>` for UI thread, `Arc<T>` for cross-thread
- Worker pattern: `mpsc::channel` for background FFT/reconstruction/file I/O -> UI
- SharedCb pattern: `Rc<RefCell<Box<dyn FnMut()>>>` for shared mutable closures
- WorkerMessage variants: FftComplete, ReconstructionComplete, AudioLoaded, WavSaved, CsvSaved, WorkerPanic
- All thread::spawn sites wrapped with catch_unwind + WorkerPanic on panic
- All mutex locks use lock_playback() helper (recovers from poisoned mutexes)

## Viewport vs Processing (Option C)
- FFT processes FULL file; sidebar Start/Stop = reconstruction time range
- Viewport zoom/scroll = visual only; "Snap to View" copies bounds to sidebar
- Time outside processing: grayed out on spectrogram. Freq cutoffs: no graying
- Waveform uses recon_start_time offset for correct viewport alignment
- Playback position = recon_start_time + audio_player.get_position_seconds()
- Dirty flag tracks settings changes; Play auto-recomputes if dirty (play_pending flag)

## Message Bar System
- `msg_bar` Frame widget in menu bar row (right of File/Analysis/Display menus)
- `set_msg(bar, MsgLevel, text)` helper in app_state.rs
- MsgLevel::Info (dimmed), Warning (yellow), Error (red)
- Used for transient feedback — separate from status_bar (persistent program status)

## Segmentation Solver
- target_segments_per_active is a one-way latch: set to Some(n) on first solver run
- When user explicitly changes window_length (dropdown or text field): clear to None
- Solver dispatches on LastEditedField: Overlap, SegmentsPerActive, BinsPerSegment
- If target is Some(n), solver overrides window_length to maintain n segments
- This was the root cause of "dropdown fails to change anything" bug

## Key Gotchas
- FLTK widget methods require `&mut self` - closures capturing widgets must be `FnMut`, not `Fn`
- Use `Rc<RefCell<Box<dyn FnMut()>>>` and call as `(cb.borrow_mut())()`
- **Spacebar — READ AGENTS.md**: Three-layer defense required. `handle()` alone does NOT work for buttons/choices/checkbuttons because FLTK's internal C++ handler bypasses it. The PRIMARY defense is `clear_visible_focus()` which prevents keyboard focus entirely.
- **NEVER use CallbackTrigger::EnterKey on text fields** — breaks validation/spacebar system. Always use Changed.
- `app::add_handler()` takes `fn(Event) -> bool` (no captures) — runs AFTER widgets, useless for blocking
- **clear_visible_focus()**: Required on ALL buttons, choices, checkbuttons, sliders, scrollbars
- **handle() vs set_callback()**: INDEPENDENT in fltk-rs. Setting one does not overwrite the other. But calling `handle()` twice DOES overwrite the first handler.
- **setup_spacebar_guards()**: MUST be called LAST in callback setup chain
- **Frame::set_label()**: Does NOT trigger repaint — must call .redraw() explicitly
- `app::event_dy()` returns `MouseWheel` enum (Up/Down/Left/Right/None), not integer
- `fltk::misc::Tooltip::enable(bool)` takes a bool; `disable()` takes no args
- `RgbImage::draw()` needs `use fltk::prelude::ImageExt`
- Module paths from submodules: use `crate::data::...` not `crate::fft_analyzer::data::...` (binary root IS the crate)
- When borrowing `st.renderer.draw(&st.data, ...)`, clone data first to avoid simultaneous mut/immut borrow
- `if let Some(ref x) = st.field { st.other = ... }` won't compile — extract into locals first, then mutate
- **FLTK Scrollbar**: `slider_size()` returns `f32` (not f64). Use generation counters to avoid fighting user drags.
- **CSV load**: Must trigger reconstruction after loading FFT from CSV, otherwise no audio/waveform.
- **GTK dialog freeze**: Disabled via app::set_option(FnfcUsesGtk, false) + FnfcUsesZenity, false
- **Rc<RefCell<AppState>> is !Send**: Extract owned data on main thread, spawn with owned data, send back via mpsc
- Build deps: needs libxft-dev, libpango1.0-dev, libxinerama-dev, libxcursor-dev, libxfixes-dev on Linux

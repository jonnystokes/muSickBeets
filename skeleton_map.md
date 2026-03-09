# skeleton_map.md -- Low-Level Code Map (FFT Analyzer)

> **Purpose:** Lightweight reference for AI agents. Combined with `map.md` (high-level), this gives complete codebase understanding without reading source files. Every public struct, field, function, enum, and macro is listed with its meaning.

---

## Entry Point: `src/fft_analyzer/main_fft.rs`

```
fn main()
  - Suppresses dbus: GIO_USE_VFS=local
  - Loads Settings from INI
  - Calls build_ui() -> (Window, Widgets)
  - Creates Rc<RefCell<AppState>>
  - Creates mpsc::channel<WorkerMessage> (tx, rx)
  - Applies Settings to AppState + Widgets
   - Creates SharedCallbacks (update_info, update_seg_label, enable_*, disable_for_processing, set_btn_*_mode)
  - Wires all callback groups:
      setup_file_callbacks, setup_rerun_callback, setup_parameter_callbacks,
      setup_display_callbacks, setup_playback_callbacks, setup_misc_callbacks,
      setup_draw_callbacks, setup_menu_callbacks, setup_scrollbar_callbacks,
      setup_zoom_callbacks, setup_snap_to_view,
      gradient_editor::setup_gradient_editor,
      setup_spacebar_handler, setup_spacebar_guards (MUST be last)
   - 16ms poll timer: checks rx for WorkerMessage, updates transport/scrubber/status
   - Five new shared callbacks defined here:
       disable_for_processing: deactivates all sidebar/transport widgets
       enable_after_processing: calls enable_audio + enable_spec + enable_wav
       set_btn_cancel_mode: red "Cancel (Space)" button
       set_btn_busy_mode: gray "Busy..." (inactive) button
       set_btn_normal_mode: blue "Recompute + Rebuild (Space)" button

Poll loop message handlers (progress refresh: 500ms / 30 ticks):
  WorkerMessage::AudioLoaded(audio, path, gain)
    -> Stores Arc<AudioData>, sets fft_params (start=0, stop=num_samples, sample_rate)
    -> Updates view bounds, enables widgets, triggers recompute via btn_rerun.do_callback()
    -> handle_audio_loaded takes shared: &SharedCallbacks parameter
  WorkerMessage::FftComplete(spectrogram)
    -> Stores Arc<Spectrogram>, computes db_ceiling from max_magnitude
    -> Spawns reconstruction worker thread (filters frames by processing time range)
    -> Records fft_duration timing
  WorkerMessage::ReconstructionComplete(audio)
    -> Stores reconstructed_audio, loads into AudioPlayer (Arc<Vec<f32>>)
    -> Updates transport state, enables WAV export, auto-plays if play_pending
    -> If lock_to_active: snaps viewport to processing range
    -> Records recon_duration timing
    -> handle_reconstruction_complete takes shared: &SharedCallbacks parameter
  WorkerMessage::CsvLoaded(result)
    -> On Ok: stores Spectrogram, applies FftParams/ReconParams/ViewParams, spawns reconstruction
    -> On Err: shows error dialog via handle_csv_load_error
    -> Dispatches to callbacks_file::handle_csv_load_result / handle_csv_load_error
  WorkerMessage::WavSaved(result) -> status bar feedback
  WorkerMessage::CsvSaved(result) -> status bar feedback
  WorkerMessage::WorkerPanic(msg) -> logs error, clears is_processing
  WorkerMessage::Cancelled(desc) -> logs, clears is_processing

All completion/error handlers call shared.enable_after_processing() + shared.set_btn_normal_mode()
```

---

## App State: `src/fft_analyzer/app_state.rs`

```
struct AppState
  audio_data: Option<Arc<AudioData>>        -- source audio (full file, mono, normalized)
  spectrogram: Option<Arc<Spectrogram>>     -- active/compat spectrogram reference
  overview_spectrogram: Option<Arc<Spectrogram>> -- whole-file overview layer
  focus_spectrogram: Option<Arc<Spectrogram>>    -- ROI high-quality layer
  overview_spec_params: Option<FftParams>   -- analysis params that produced overview layer
  focus_spec_params: Option<FftParams>      -- analysis params that produced focus layer
  fft_params: FftParams                     -- analysis parameters
  overview_fft_defaults: FftParams          -- separate defaults for whole-file overview layer
  view: ViewState                           -- viewport + display + recon params
  transport: TransportState                 -- playback position/state

  audio_player: AudioPlayer                 -- miniaudio playback device
  spec_renderer: SpectrogramRenderer        -- cached spectrogram RGB image
  overview_spec_renderer: SpectrogramRenderer -- cached whole-file overview image
  focus_spec_renderer: SpectrogramRenderer  -- cached focused ROI image
  wave_renderer: WaveformRenderer           -- cached waveform RGB image

  reconstructed_audio: Option<AudioData>    -- inverse-FFT result (time range only)
  recon_start_sample: usize                 -- where reconstruction starts in source timeline
  is_processing: bool                       -- blocks re-entry during FFT/recon
  dirty: bool                               -- params changed since last recompute
  play_pending: bool                        -- auto-play after next reconstruction
  lock_to_active: bool                      -- auto-snap viewport after recompute
  has_audio: bool                           -- any audio loaded
  current_filename: String

  tooltip_mgr: TooltipManager
  time_zoom_factor: f32                     -- button zoom multiplier (default 1.5)
  freq_zoom_factor: f32                     -- button zoom multiplier (default 1.5)
  mouse_zoom_factor: f32                    -- scroll wheel zoom (default 1.2)
  swap_zoom_axes: bool                      -- swap Alt vs Alt+Ctrl zoom target
  normalize_audio: bool                     -- auto-normalize on load
  normalize_peak: f32                       -- normalization target (default 0.97)
  source_norm_gain: f32                     -- applied gain (to recover original)
  cancel_flag: Arc<AtomicBool>              -- cancels in-flight workers
  progress_counter: Arc<AtomicUsize>        -- shared with worker threads for progress reporting
  progress_total: usize                     -- total items for current operation
  status: StatusBarManager                  -- consolidated status text, activity, and operation timing

fn new_cancel_flag() -> Arc<AtomicBool>     -- cancels old, creates fresh flag
fn recon_start_seconds() -> f64             -- recon_start_sample / sample_rate
fn derived_info() -> DerivedInfo            -- computes segments, bins, hop, freq_res from current params
fn active_spectrogram() -> Option<Arc<Spectrogram>> -- focus first, then overview, then compat fallback
fn invalidate_all_spectrogram_renderers()   -- invalidates overview/focus/compat render caches
fn overview_params_for_audio(total_samples) -> FftParams -- whole-file params from overview defaults

struct TimedEntry                           -- (private) single recorded timing
  key: String                               -- label (e.g. "FFT", "Recon")
  duration: Duration
  recorded_at: Instant                      -- for potential expiry/ordering

struct StatusBarManager                     -- consolidates status bar state (replaces scattered timing fields)
  activity: String                          -- current activity ("Ready", "Processing FFT...", etc.)
  progress: Option<String>                  -- optional progress detail
  timings: VecDeque<TimedEntry>             -- recorded operation timings (ordered)
  operation_start: Option<(String, Instant)> -- in-flight timed operation

fn new() -> Self                            -- activity="Ready", empty timings
fn set_activity(&mut self, &str)            -- update current activity label
fn set_progress(&mut self, Option<&str>)    -- set or clear progress detail
fn record_timing(&mut self, &str, Duration) -- add/update a named timing entry
fn start_timing(&mut self, &str)            -- begin timing a named operation
fn finish_timing(&mut self) -> Option<Duration> -- end timing, record result, return duration
fn cancel_timing(&mut self)                 -- discard in-flight timing without recording
fn clear_timings(&mut self)                 -- remove all recorded timings
fn render(&self) -> String                  -- "Ready | FFT: 32.6s | Recon: 4.8s | Memory: 1.2 GB"
fn render_wrapped(max_chars: usize) -> String -- renders with line breaks at | boundaries
fn measure_height(win_width: i32) -> i32    -- estimates pixel height needed for wrapped status

struct DerivedInfo { total_samples, freq_bins, freq_resolution, hop_length, segments, bin_duration_ms, window_length, sample_rate, overlap_percent }
  fn format_info() -> String                -- compact multi-line info
  fn format_segmentation_sentence() -> String -- prose description
  fn format_resolution() -> String          -- window/freq/time resolution summary

struct UpdateThrottle { last_update, min_interval }
  fn should_update() -> bool                -- rate-limit redraws (e.g. 50ms)

type SharedCb = Rc<RefCell<Box<dyn FnMut()>>>
struct SharedCallbacks { update_info, update_seg_label, enable_audio_widgets, enable_spec_widgets, enable_wav_export,
  disable_for_processing, enable_after_processing, set_btn_cancel_mode, set_btn_busy_mode, set_btn_normal_mode }

enum MsgLevel { Info, Warning, Error }
fn set_msg(bar, level, text)                -- color-coded message on top bar
fn update_status_bar(status_bar: &mut MultilineOutput, text) -- bottom status bar + flush
fn format_time(seconds) -> String           -- "M:SS.sssss"
fn format_memory_usage() -> String          -- reads /proc/self/status VmRSS
```

---

## Data Models: `src/fft_analyzer/data/`

### `fft_params.rs`
```
enum WindowType { Hann, Hamming, Blackman, Kaiser(f32) }
enum TimeUnit { Seconds, Samples }

struct FftParams
  window_length: usize                      -- segment size in samples (must be even)
  overlap_percent: f32                      -- 0..99
  window_type: WindowType
  use_center: bool                          -- zero-pad signal edges
  start_sample, stop_sample: usize          -- processing range (ground truth, always samples)
  time_unit: TimeUnit                       -- UI display only
  sample_rate: u32
  zero_pad_factor: usize                    -- 1/2/4/8x
  target_segments_per_active: Option<usize> -- solver target
  target_bins_per_segment: Option<usize>    -- solver target
  last_edited_field: LastEditedField        -- which field solver should prioritize

fn hop_length() -> usize                    -- window_length * (1 - overlap/100)
fn n_fft_padded() -> usize                  -- window_length * zero_pad_factor
fn start_seconds(), stop_seconds() -> f64
fn num_frequency_bins() -> usize            -- n_fft_padded/2 + 1
fn frequency_resolution() -> f32            -- sample_rate / n_fft_padded
fn num_segments(total_samples) -> usize     -- accounts for use_center padding
fn bin_duration_seconds() -> f64            -- hop / sample_rate
fn generate_window() -> Vec<f32>            -- applies WindowType to produce window coefficients
```

### `view_state.rs`
```
struct GradientStop { position: f32, r: f32, g: f32, b: f32 }
fn default_custom_gradient() -> Vec<GradientStop>   -- SebLague rainbow
fn eval_gradient(stops, t) -> (f32, f32, f32)       -- linear interp between stops

enum FreqScale { Linear, Log, Power(f32) }   -- Power(0)=linear, Power(1)=log, between=blend
enum ColormapId { Classic, Viridis, Magma, Inferno, Greyscale, InvertedGrey, Geek, Custom }
  const ALL, fn name(), fn from_index()

struct ViewState
  -- Viewport:
  freq_min_hz, freq_max_hz: f32             -- visible frequency range
  freq_scale: FreqScale
  time_min_sec, time_max_sec: f64           -- visible time range

  -- Display:
  threshold_db: f32                         -- min dB to show (default -87)
  db_ceiling: f32                           -- max dB for colormap (auto-set from data)
  brightness: f32, gamma: f32
  colormap: ColormapId
  custom_gradient: Vec<GradientStop>

  -- Reconstruction:
  recon_freq_count: usize                   -- top-N bins per frame
  recon_freq_min_hz, recon_freq_max_hz: f32 -- bandpass for reconstruction

  -- Data bounds (full file):
  data_freq_max_hz, data_time_min_sec, data_time_max_sec, max_freq_bins

fn y_to_freq(t: f32) -> f32                -- normalized [0,1] -> Hz (handles all scale modes)
fn freq_to_y(freq: f32) -> f32             -- Hz -> normalized [0,1] (binary search for Power blend)
fn x_to_time(t: f64) -> f64               -- normalized -> seconds
fn time_to_x(sec: f64) -> f64             -- seconds -> normalized
fn reset_zoom()                            -- snap to full data bounds
fn visible_time_range(), visible_freq_range()

struct TransportState { position_samples, duration_samples, sample_rate, is_playing, repeat }
```

### `spectrogram.rs`
```
struct FftFrame { time_seconds: f64, magnitudes: Vec<f32>, phases: Vec<f32> }

struct Spectrogram
  frames: Vec<FftFrame>                     -- sorted by time
  frequencies: Vec<f32>                     -- shared freq vector (bin_idx * freq_res)
  max_freq, min_time, max_time

fn from_frames_with_frequencies(frames, frequencies)  -- sorts frames, computes bounds
fn num_frames(), num_bins()
fn magnitude_to_db(mag) -> f32              -- 20 * log10(mag)
fn bin_at_freq(freq_hz) -> Option<usize>    -- binary search
fn max_magnitude() -> f32
fn frame_at_time(seconds) -> Option<usize>  -- binary search

fn compute_active_bins(magnitudes, frequencies, freq_min, freq_max, top_n) -> Vec<usize>
  -- Shared filter: freq range -> sort by magnitude -> top N. Used by renderer + reconstructor.
```

### `audio_data.rs`
```
struct AudioData { samples: Arc<Vec<f32>>, sample_rate: u32, duration_seconds: f64 }

fn from_wav_file(path) -> Result<Self>      -- supports 8/16/24/32 PCM + float, downmixes to mono
fn save_wav(path)                           -- 16-bit PCM output
fn num_samples(), get_slice(start, end), nyquist_freq()
fn normalize(target_peak) -> f32            -- returns gain applied; uses Arc::make_mut if needed
```

### `segmentation_solver.rs`
```
enum LastEditedField { Overlap, SegmentsPerActive, BinsPerSegment }

struct SolverConstraints { min_window(4), max_window, min_overlap_percent(0), max_overlap_percent(99) }
struct SolverInput { active_samples, window_length, overlap_percent, zero_pad_factor, target_segments, target_bins, last_edited, constraints }
struct SolverOutput { window_length, overlap_percent, segments_per_active, bins_per_segment }

fn solve(input) -> SolverOutput
  -- SegmentsPerActive: keeps overlap fixed, solves for window_length
  -- Overlap: if target_segments locked, solves window to match
  -- BinsPerSegment: if target_segments locked, uses that; else derives window from bins

fn solve_window_for_segments(active, overlap, target, constraints) -> usize
  -- Analytical approximation + local search (256 steps) for closest segment count

fn clamp_even(value, min, max) -> usize     -- forces even, minimum 4
fn hop_length(window, overlap) -> usize
fn num_segments(active, window, hop) -> usize
```

---

## Processing: `src/fft_analyzer/processing/`

### `fft_engine.rs`
```
thread_local FFT_PLANNER: RefCell<RealFftPlanner<f32>>  -- per-rayon-thread cache

struct FftEngine (stateless)
fn process(audio, params, cancel, progress: Option<&AtomicUsize>) -> Spectrogram
  1. Slices audio[start_sample..stop_sample]
  2. Optionally center-pads (window_len/2 zeros each side)
  3. Computes shared frequencies vector once
  4. Rayon par_iter over frames:
     - Check cancel flag
     - Window segment, zero-pad to n_fft
     - Forward realfft
     - Extract magnitude (normalized: |X|/N * amplitude_scale) and phase
  5. Builds Spectrogram from frames + frequencies
```

### `reconstructor.rs`
```
thread_local IFFT_PLANNER: RefCell<RealFftPlanner<f32>>

struct Reconstructor (stateless)
fn reconstruct(spec, params, view, cancel, progress: Option<&AtomicUsize>) -> AudioData       -- full spectrogram
fn reconstruct_range(spec, params, view, frame_range, cancel, progress: Option<&AtomicUsize>) -> AudioData
  Phase 1 (parallel): For each frame in range:
    - Filter bins by [recon_freq_min, recon_freq_max]
    - Sort by magnitude, keep top recon_freq_count
    - Undo forward scaling (DC/Nyquist: mag*N, others: mag*N/2)
    - Inverse realfft, normalize by 1/N
    - Apply synthesis window (first window_len samples)
  Phase 2 (sequential): Overlap-add
    - Accumulate windowed frames + window_sum (w[i]^2)
    - Normalize: output[i] /= window_sum[i], with 10%-of-peak threshold to avoid edge artifacts
```

---

## Rendering: `src/fft_analyzer/rendering/`

### `spectrogram_renderer.rs`
```
struct SpectrogramRenderer
  color_lut: ColorLUT
  cached_image: Option<RgbImage>, cached_buffer: Vec<u8>
  cache_valid: bool, last_widget_size, last_view_hash: u64

fn invalidate()                             -- force rebuild next draw
fn update_lut(view)                         -- sync LUT params from ViewState
fn draw(spec, view, proc_time_min/max, x, y, w, h)
  - Hash-based cache: skip rebuild if nothing changed
  - rebuild_cache():
    1. Compute active_bins per frame (parallel): mirrors reconstructor's freq+count filter
    2. Pre-compute row_bins[]: pixel row -> nearest freq bin (binary search)
    3. Pre-compute col_data[]: pixel col -> (frame_idx, time)
    4. Parallel row rendering: for each (row, col), lookup magnitude, LUT color
       - Out-of-processing-range: desaturate + dim to 35%
    5. Create RgbImage from buffer
```

### `waveform_renderer.rs`
```
struct WaveformRenderer
  cached_image, cached_buffer, cache_valid, last_size, last_view_hash

fn draw(samples, sample_rate, audio_time_start, view, cursor_x, x, y, w, h)
  - Hash-based cache
  - Dual mode based on samples_per_pixel:
    >4: draw_peaks() -- rayon parallel min/max per column
    <=4: draw_samples() -- Bresenham lines between samples, 3x3 dots when very zoomed
  - Playback cursor drawn on top via FLTK draw calls (not cached)

fn draw_line(x0,y0,x1,y1,...) -- Bresenham's algorithm on pixel buffer
fn set_pixel(x, y, width, color)
```

### `color_lut.rs`
```
const LUT_SIZE: 1024

struct ColorLUT
  table: Vec<(u8,u8,u8)>                   -- precomputed lookup table
  threshold_db, db_ceiling, brightness, gamma, colormap
  custom_stops: Vec<GradientStop>

fn rebuild()                                -- fills table: t -> gamma -> brightness -> colormap
fn set_params(...) -> bool                  -- returns true if LUT was rebuilt
fn set_custom_stops(stops) -> bool          -- only rebuilds if colormap == Custom
fn lookup(magnitude) -> (u8,u8,u8)          -- mag -> dB -> normalize [threshold,ceiling] -> LUT index

Colormaps (all fn(t:f32) -> (u8,u8,u8)):
  classic: SebLague 7-stop gradient (Black->Purple->Blue->Green->Yellow->Orange->Red)
  viridis, magma, inferno: polynomial approximations
  greyscale, inverted_grey: linear ramps
  geek: black->dark green->light green->white (3-segment piecewise)
  custom: eval_gradient() from user stops
```

---

## Layout: `src/fft_analyzer/layout.rs` + `layout_sidebar.rs`

```
Constants: WIN_W=1200, WIN_H=1555, MENU_H=25, STATUS_H=25, SIDEBAR_W=215,
  SPEC_LEFT_GUTTER_W=50, SPEC_RIGHT_GUTTER_W=20
Note: status_bar widget is MultilineOutput (supports wrapped status text)

struct Widgets -- 70+ cloneable handles to every UI widget

fn build_ui() -> (Window, Widgets)
  Layout: menu_row (MenuBar + msg_bar) | root Flex row:
    Left: scrollable sidebar column -> delegates to layout_sidebar::build_sidebar()
    Right: column (waveform_row [left spacer 50px | waveform | right spacer 20px]
           | spec_row [freq_axis 50px | spectrogram | freq_zoom+scrollbar 20px]
           | time_axis_row [left spacer 50px | time_axis | right spacer 20px]
           | time_zoom_row 20px | scrub_row [left spacer 50px | scrubber | right spacer 20px]
           | transport_row 28px)
  Shared spectrogram gutter constants keep waveform/scrubber/time-axis width aligned
  to the drawable spectrogram width as the window resizes.
  Below root: status_fft (absolute pos) | status_bar (absolute pos)

--- layout_sidebar.rs ---
struct SidebarWidgets -- all sidebar widget handles returned to build_ui

fn build_sidebar(left: &mut Flex) -> SidebarWidgets
  Sections: FILE (open/save/load/export) | ANALYSIS (time range, segment size, overlap,
    segments/active, bins/segment, window type, kaiser beta, center/pad, zero-pad, resolution info,
    rerun button) | DISPLAY (colormap, gradient preview, scale/threshold/ceiling/brightness/gamma
    sliders) | RECONSTRUCTION (freq count, recon freq min/max, snap to view) | INFO (multiline
    output) | Settings (tooltips toggle, lock to active, home, save defaults)
```

---

## Callbacks: `src/fft_analyzer/callbacks_*.rs`

### `callbacks_file.rs`
```
fn setup_file_callbacks(widgets, state, tx, shared, win)
  setup_open_callback: file dialog -> disable_for_processing -> set_btn_busy_mode -> spawn thread -> AudioData::from_wav_file -> normalize -> tx.send(AudioLoaded)
  setup_save_fft_callback: file dialog -> disable_for_processing -> set_btn_busy_mode -> spawn thread -> csv_export::export_to_csv -> tx.send(CsvSaved)
  setup_load_fft_callback: file dialog -> disable_for_processing -> set_btn_busy_mode -> spawn thread -> csv_import -> tx.send(CsvLoaded)
  setup_save_wav_callback: file dialog -> disable_for_processing -> set_btn_busy_mode -> spawn thread -> audio.save_wav -> tx.send(WavSaved)
  All file operations call disable_for_processing + set_btn_busy_mode on start
  CSV load I/O now runs in a background thread (was blocking UI)

fn setup_rerun_callback(widgets, state, tx, shared)
  - If is_processing: triggers cancel_flag (rerun button acts as cancel during processing)
  - Calls disable_for_processing + set_btn_cancel_mode on start
  - Syncs all UI fields into FftParams (time range, window, overlap, window_type, zero-pad, recon params)
  - Overrides start/stop to process FULL file for FFT
  - Spawns FFT worker thread -> tx.send(FftComplete)
  - Memory warning if zero-padded size > 256MB estimated

fn handle_csv_load_result(result, state, widgets, tx, shared)
  - On Ok: stores Spectrogram, applies FftParams/ReconParams/ViewParams, spawns reconstruction
  - On Err: delegates to handle_csv_load_error

fn handle_csv_load_error(error_msg)
  - Shows FLTK error dialog for failed CSV load
```

### `callbacks_ui.rs`
```
fn setup_parameter_callbacks(widgets, state, shared)
  - Time unit toggle (Seconds <-> Samples, converts display values)
  - Overlap slider -> sets last_edited=Overlap, runs solver, updates derived fields
  - Segments/Active input -> sets last_edited=SegmentsPerActive, runs solver
  - Bins/Segment input -> sets last_edited=BinsPerSegment, runs solver
  - Window type dropdown -> updates fft_params.window_type, shows/hides kaiser beta
  - Seg preset dropdown -> sets window_length, clears solver targets
  - Seg size text input -> validates even, clears solver targets
  - Center/Pad checkbox, Zero-pad dropdown
  All text inputs: CallbackTrigger::Changed, suppress_solver_inputs guard prevents loops

fn apply_segmentation_solver(st) -- calls solver::solve(), writes back to FftParams, sets dirty=true
fn current_active_samples(st) -> usize

fn setup_display_callbacks(widgets, state)
  - Colormap dropdown, freq scale slider, threshold/ceiling/brightness/gamma sliders
  - Each: updates ViewState, invalidates renderer, redraws (throttled 50ms for sliders)

fn setup_playback_callbacks(widgets, state)
  - Play: if dirty -> set play_pending + rerun; else audio_player.play()
  - Pause, Stop, Scrub slider (Push/Drag/Released seeking), Repeat

fn setup_misc_callbacks(widgets, state, win)
  - Tooltips toggle, Lock-to-Active toggle
  - Home button: snaps viewport to processing time + recon freq range
  - Save As Default: Settings::from_app_state() -> save to INI
```

### `gradient_editor.rs`
```
struct GradientEditorState { selected_stop: Option<usize>, dragging: bool }

const GRAD_BAR_H: 20, STOP_HANDLE_H: 10, GRAD_MARGIN: 4

fn setup_gradient_editor(widgets, state)
  - Draw callback: renders gradient bar pixel-by-pixel + stop handles as triangles
  - Handle callback: left-click add/select stop, drag to move, right-click delete, double-click color picker
  - Spacebar guard: blocks KeyDown/Shortcut, triggers recompute on KeyUp
  - Only interactive when colormap == Custom

fn find_stop_at_x(stops, pos_t, bar_w) -> Option<usize>
  - Finds nearest stop within 6-pixel tolerance
```

### `callbacks_nav.rs`
```
fn setup_menu_callbacks(widgets, state)
  - File: Open(Ctrl+O), Save FFT(Ctrl+S), Load FFT(Ctrl+L), Export WAV(Ctrl+E), Quit(Ctrl+Q)
  - Analysis: Recompute FFT
  - Display: Reset Zoom

fn setup_scrollbar_callbacks(widgets, state) -> (Rc<Cell<u64>>, Rc<Cell<u64>>)
  - X scroll: pans time (0..10000 range mapped to data bounds)
  - Y scroll: pans frequency (inverted for vertical)
  - Returns generation counters to prevent poll loop from fighting user drags

fn setup_zoom_callbacks(widgets, state)
  - Time +/-: zoom centered, clamped to data bounds
  - Freq +/-: zoom centered, min 10 Hz range

fn setup_snap_to_view(widgets, state)
  - Copies viewport bounds -> start/stop + recon freq min/max, triggers recompute

fn setup_spacebar_handler(win, widgets)
  - Window-level: consume KeyDown, trigger recompute on KeyUp, consume Shortcut

fn setup_spacebar_guards(widgets)  -- MUST be called last
  - Macro block_space!: attaches handle() to ALL buttons, choices, checkboxes, sliders, scrollbars
  - clear_visible_focus() on all non-text widgets
  - Re-attaches text input validation with recompute trigger
```

### `callbacks_draw.rs`
```
fn setup_draw_callbacks(widgets, state)

setup_spectrogram_draw: try_borrow_mut -> spec_renderer.draw() -> playback cursor line
setup_spectrogram_mouse:
  - Push/Drag: click-to-seek (converts pixel -> time -> audio position)
  - Move: hover readout (freq, dB, time) in cursor_readout label
  - MouseWheel: no mod=pan freq, Ctrl=pan time, Alt=zoom freq, Alt+Ctrl=zoom time (swap_zoom_axes swaps Alt targets)
  - Released: end seeking
  - Leave: clear readout

setup_waveform_draw: try_borrow_mut -> take/put reconstructed_audio -> wave_renderer.draw() -> put back

setup_freq_axis_draw:
  - generate_freq_ticks(): pixel-space-first approach
    1. Even Y spacing -> 2. y_to_freq inverse (binary search) -> 3. round to nice -> 4. dedup -> 5. freq_to_y
  - Dashed yellow lines for recon freq boundaries

setup_time_axis_draw:
  - Adaptive labels: target 1 per 80px, snap to nice_steps[]
  - Dashed yellow lines for processing time boundaries
```

---

## Playback: `src/fft_analyzer/playback/audio_player.rs`

```
enum PlaybackState { Stopped, Playing, Paused }

struct AudioPlayer
  device: Option<Device>                    -- miniaudio playback device
  device_sample_rate: u32
  playback_data: Arc<Mutex<PlaybackData>>

struct PlaybackData { samples: Arc<Vec<f32>>, sample_rate, position, state, repeat, end_sample, is_seeking }

fn lock_playback(mutex) -> MutexGuard       -- recovers from poison (never panics)
fn load_audio(samples: Arc<Vec<f32>>, sample_rate) -- recreates device if sample rate changed
fn init_device(sample_rate)                 -- miniaudio F32 mono, data callback fills from PlaybackData
  Data callback: if Playing, copies samples[position..], handles repeat/end/seeking
fn play(), pause(), stop()
fn seek_to(seconds), seek_to_sample(sample)
fn set_seeking(bool), set_repeat(bool)
fn get_state(), get_position_samples(), get_position_seconds(), has_audio()
```

---

## Support Modules

### `src/fft_analyzer/csv_export.rs`
```
fn export_to_csv(spec, params, view, path, time_filter) -> Result
fn import_from_csv(path) -> Result<(Spectrogram, FftParams, Option<recon_params>, ViewParams)>
  -- Header with metadata (#sample_rate, #window_length, etc.), then frame-per-row CSV
```

### `src/fft_analyzer/settings.rs`
```
struct Settings -- mirrors AppState fields for INI persistence
fn load() -> Self                           -- reads settings.ini or defaults
fn save()
fn from_app_state(state)
fn apply_to_state(state, widgets)           -- restores all UI state from INI
  -- Also stores: window_width, window_height, solver targets, custom gradient
```

### `src/fft_analyzer/validation.rs`
```
fn attach_float_validation(input)           -- blocks non-numeric chars via handle()
fn attach_uint_validation(input)            -- blocks non-digit chars
fn attach_float_validation_with_recompute(input, btn_rerun)  -- + space triggers recompute
fn attach_uint_validation_with_recompute(input, btn_rerun)
fn parse_or_zero_f32, parse_or_zero_f64, parse_or_zero_usize  -- safe parsers
```

### `src/fft_analyzer/ui/`
```
theme.rs: Color constants (BG_DARK, BG_PANEL, BG_WIDGET, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_DISABLED, ACCENT_*, BORDER, SEPARATOR)
  fn color(hex) -> Color, fn accent_color(), fn section_header_color()

tooltips.rs: struct TooltipManager { enabled }
  fn set_enabled(bool), fn set_tooltip(widget, text)
```

### `src/fft_analyzer/debug_flags.rs`
```
macro app_log!(tag, fmt, args...)           -- always prints to stderr with timestamp
macro dbg_log!(flag, tag, fmt, args...)     -- only prints if flag is true
const CURSOR_DBG: bool      -- mouse cursor position/freq/dB readout
const FFT_DBG: bool         -- FFT processing pipeline, worker lifecycle, timing
const PLAYBACK_DBG: bool    -- audio playback state transitions, seek, loop
const RENDER_DBG: bool      -- spectrogram/waveform draw calls, cache hits/misses
const FILE_IO_DBG: bool     -- file open/save/load operations with counts (default: true)
fn instant_since_start(Instant) -> String   -- elapsed since program start
```

---

## Known Issues / Gaps (from full code review)

1. ~~**Duplicate active-bin logic**~~: FIXED -- extracted to shared `compute_active_bins()` in `spectrogram.rs`.
2. ~~**Sample buffer clone**~~: FIXED -- `AudioData.samples` is `Arc<Vec<f32>>`, so reconstructed audio and `AudioPlayer` now share the same allocation without cloning.
3. ~~**Absolute-positioned status bars**~~: FIXED -- `Event::Resize` handler in `setup_spacebar_handler()` repositions them on window resize.
5. ~~**No progress indication**~~: FIXED -- `progress_counter: Arc<AtomicUsize>` in AppState is shared with FFT/reconstruction workers, which increment it per frame. Poll loop reads the counter and updates status bar with percentage.

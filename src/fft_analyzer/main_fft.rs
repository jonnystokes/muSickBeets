#[macro_use]
mod debug_flags;
mod app_state;
mod callbacks_draw;
mod callbacks_file;
mod callbacks_nav;
mod callbacks_ui;
mod csv_export;
mod data;
mod gradient_editor;
mod layout;
mod layout_sidebar;
mod playback;
mod poll_loop;
mod processing;
mod rendering;
mod settings;
mod ui;
mod validation;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;

use fltk::{app, prelude::*};

use app_state::{AppState, SharedCallbacks, SharedCb, WorkerMessage};
use layout::{STATUS_FFT_OFFSET, Widgets};

// ═══════════════════════════════════════════════════════════════════════════
//  CREATE SHARED CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

fn create_shared_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
    win: &fltk::window::Window,
) -> SharedCallbacks {
    // Track whether the user has manually edited the freq count field.
    // If not, it always syncs to max bins. If yes, it only clamps down.
    let freq_count_user_adjusted = Rc::new(Cell::new(false));

    // Set callback on the freq count input to detect manual edits
    {
        let flag = freq_count_user_adjusted.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        input_freq_count.set_trigger(fltk::enums::CallbackTrigger::Changed);
        input_freq_count.set_callback(move |inp| {
            if inp.value().contains(' ') {
                inp.set_value(&inp.value().replace(' ', ""));
                return;
            }
            flag.set(true);
        });
    }

    let update_info: SharedCb = {
        let state = state.clone();
        let mut lbl_info = widgets.lbl_info.clone();
        let mut lbl_resolution_info = widgets.lbl_resolution_info.clone();
        let mut lbl_hop_info = widgets.lbl_hop_info.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        let mut input_segments_per_active = widgets.input_segments_per_active.clone();
        let mut input_bins_per_segment = widgets.input_bins_per_segment.clone();
        let mut status_fft = widgets.status_fft.clone();
        let mut status_bar = widgets.status_bar.clone();
        let mut root = widgets.root.clone();
        let win = win.clone();
        let flag = freq_count_user_adjusted.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let info = match state.try_borrow() {
                Ok(st) => st.derived_info(),
                Err(_) => return,
            };
            lbl_info.set_value(&info.format_info());
            lbl_resolution_info.set_value(&info.format_resolution());

            // Update hop display
            let hop_ms = info.hop_length as f64 / info.sample_rate.max(1) as f64 * 1000.0;
            lbl_hop_info.set_label(&format!("Hop: {} smp ({:.1} ms)", info.hop_length, hop_ms));

            let current: usize = input_freq_count.value().parse().unwrap_or(info.freq_bins);
            if !flag.get() {
                // User hasn't manually adjusted: always track max
                input_freq_count.set_value(&info.freq_bins.to_string());
            } else if current > info.freq_bins {
                // User adjusted, but current exceeds new max: clamp down
                input_freq_count.set_value(&info.freq_bins.to_string());
            }

            // Avoid clobbering active in-progress edits in the segmentation fields.
            // They are updated by their own callbacks once a valid value is committed.
            if !input_segments_per_active.has_focus() {
                input_segments_per_active.set_value(&info.segments.to_string());
            }
            if !input_bins_per_segment.has_focus() {
                input_bins_per_segment.set_value(&info.freq_bins.to_string());
            }

            let sentence = info.format_segmentation_sentence();
            status_fft.set_value(&sentence);
            let width_chars = ((win.w() - 16).max(40) / 7).max(20) as usize;
            let line_count = sentence
                .split('\n')
                .map(|line| ((line.chars().count().max(1) - 1) / width_chars) + 1)
                .sum::<usize>()
                .max(1) as i32;
            let fft_h = (line_count * 17 + 8).max(24);
            let base_h = 25;
            let menu_h = 25;
            let win_h = win.h();
            let win_w = win.w();
            root.resize(
                0,
                menu_h,
                win_w,
                win_h - menu_h - base_h - fft_h - STATUS_FFT_OFFSET,
            );
            status_fft.resize(0, win_h - base_h - fft_h - STATUS_FFT_OFFSET, win_w, fft_h);
            status_bar.resize(0, win_h - base_h, win_w, base_h);
        })))
    };

    let update_seg_label: SharedCb = {
        let state = state.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        Rc::new(RefCell::new(Box::new(move || {
            let st = match state.try_borrow() {
                Ok(st) => st,
                Err(_) => return,
            };
            let wl = st.fft_params.window_length;
            if !input_seg_size.has_focus() {
                input_seg_size.set_value(&wl.to_string());
            }
            // Sync preset dropdown
            let preset_idx = match wl {
                256 => 0,
                512 => 1,
                1024 => 2,
                2048 => 3,
                4096 => 4,
                8192 => 5,
                16384 => 6,
                32768 => 7,
                65536 => 8,
                _ => 9, // Custom
            };
            seg_preset_choice.set_value(preset_idx);
        })))
    };

    let enable_audio_widgets: SharedCb = {
        let mut btn_time_unit = widgets.btn_time_unit.clone();
        let mut input_start = widgets.input_start.clone();
        let mut input_stop = widgets.input_stop.clone();
        let mut input_seg_size = widgets.input_seg_size.clone();
        let mut seg_preset_choice = widgets.seg_preset_choice.clone();
        let mut slider_overlap = widgets.slider_overlap.clone();
        let mut input_segments_per_active = widgets.input_segments_per_active.clone();
        let mut input_bins_per_segment = widgets.input_bins_per_segment.clone();
        let mut window_type_choice = widgets.window_type_choice.clone();
        let mut check_center = widgets.check_center.clone();
        let mut zero_pad_choice = widgets.zero_pad_choice.clone();
        let mut btn_rerun = widgets.btn_rerun.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_time_unit.activate();
            input_start.activate();
            input_stop.activate();
            input_seg_size.activate();
            seg_preset_choice.activate();
            slider_overlap.activate();
            input_segments_per_active.activate();
            input_bins_per_segment.activate();
            window_type_choice.activate();
            check_center.activate();
            zero_pad_choice.activate();
            btn_rerun.activate();
        })))
    };

    let enable_spec_widgets: SharedCb = {
        let mut btn_save_fft = widgets.btn_save_fft.clone();
        let mut input_freq_count = widgets.input_freq_count.clone();
        let mut input_recon_freq_min = widgets.input_recon_freq_min.clone();
        let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
        let mut btn_play = widgets.btn_play.clone();
        let mut btn_pause = widgets.btn_pause.clone();
        let mut btn_stop = widgets.btn_stop.clone();
        let mut scrub_slider = widgets.scrub_slider.clone();
        let mut repeat_choice = widgets.repeat_choice.clone();
        let mut btn_snap_to_view = widgets.btn_snap_to_view.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_save_fft.activate();
            input_freq_count.activate();
            input_recon_freq_min.activate();
            input_recon_freq_max.activate();
            btn_play.activate();
            btn_pause.activate();
            btn_stop.activate();
            scrub_slider.activate();
            repeat_choice.activate();
            btn_snap_to_view.activate();
        })))
    };

    let enable_wav_export: SharedCb = {
        let mut btn_save_wav = widgets.btn_save_wav.clone();
        Rc::new(RefCell::new(Box::new(move || {
            btn_save_wav.activate();
        })))
    };

    SharedCallbacks {
        update_info,
        update_seg_label,
        enable_audio_widgets,
        enable_spec_widgets,
        enable_wav_export,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  MAIN
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    // Disable GTK native file dialogs — they depend on dbus/GVFS volume monitors
    // which hang or freeze in environments without a full GNOME session
    // (Termux chroot, VNC, WSL, containers, etc.). FLTK's own file chooser
    // is used instead, which works reliably everywhere.
    app::set_option(app::Option::FnfcUsesGtk, false);
    app::set_option(app::Option::FnfcUsesZenity, false);

    // Also suppress any residual GVFS warnings from GTK libraries loaded elsewhere.
    // SAFETY: called at the very start of main, before any other threads exist.
    unsafe {
        std::env::set_var("GIO_USE_VFS", "local");
        std::env::set_var("GIO_USE_VOLUME_MONITOR", "unix");
        std::env::set_var("GVFS_REMOTE_VOLUME_MONITOR_IGNORE", "1");
    }

    // Load settings from INI (or create default INI if missing)
    let cfg = settings::Settings::load_or_create();
    app_log!(
        "Settings",
        "Loaded: recon_freq_max={}Hz, view_freq={}-{}Hz, window={}x{}",
        cfg.recon_freq_max_hz,
        cfg.view_freq_min_hz,
        cfg.view_freq_max_hz,
        cfg.window_width,
        cfg.window_height
    );

    let app = app::App::default();

    // Apply dark theme
    ui::theme::apply_dark_theme();
    app::set_visual(fltk::enums::Mode::Rgb8).ok();

    let (mut win, widgets) = layout::build_ui();

    // Apply window size from settings
    win.set_size(cfg.window_width, cfg.window_height);

    // Apply settings to state
    let state = {
        let mut st = AppState::new();
        st.fft_params.window_length = cfg.window_length;
        st.fft_params.overlap_percent = cfg.overlap_percent;
        st.fft_params.use_center = cfg.center_pad;
        st.view.freq_min_hz = cfg.view_freq_min_hz;
        st.view.freq_max_hz = cfg.view_freq_max_hz;
        st.view.freq_scale = data::FreqScale::Power(cfg.freq_scale_power);
        st.view.threshold_db = cfg.threshold_db;
        st.view.brightness = cfg.brightness;
        st.view.gamma = cfg.gamma;
        st.view.colormap = data::ColormapId::from_index(cfg.colormap_index());
        st.view.custom_gradient = cfg.parse_custom_gradient();
        st.view.recon_freq_min_hz = cfg.recon_freq_min_hz;
        st.view.recon_freq_max_hz = cfg.recon_freq_max_hz;
        st.view.recon_freq_count = cfg.recon_freq_count;
        st.lock_to_active = cfg.lock_to_active;
        st.time_zoom_factor = cfg.time_zoom_factor;
        st.freq_zoom_factor = cfg.freq_zoom_factor;
        st.mouse_zoom_factor = cfg.mouse_zoom_factor;
        st.swap_zoom_axes = cfg.swap_zoom_axes;
        st.normalize_audio = cfg.normalize_audio;
        st.normalize_peak = cfg.normalize_peak;
        st.view.db_ceiling = cfg.db_ceiling;
        st.fft_params.zero_pad_factor = cfg.zero_pad_factor;
        st.fft_params.target_segments_per_active = if cfg.target_segments_per_active > 0 {
            Some(cfg.target_segments_per_active)
        } else {
            None
        };
        st.fft_params.target_bins_per_segment = if cfg.target_bins_per_segment > 0 {
            Some(cfg.target_bins_per_segment)
        } else {
            None
        };
        st.fft_params.last_edited_field = match cfg.last_edited_field.as_str() {
            "SegmentsPerActive" => data::LastEditedField::SegmentsPerActive,
            "BinsPerSegment" => data::LastEditedField::BinsPerSegment,
            _ => data::LastEditedField::Overlap,
        };
        Rc::new(RefCell::new(st))
    };
    let (tx, rx) = mpsc::channel::<WorkerMessage>();

    // Create shared callbacks
    let shared = create_shared_callbacks(&widgets, &state, &win);

    // Wire up all callbacks
    callbacks_nav::setup_menu_callbacks(&widgets, &state);
    callbacks_file::setup_file_callbacks(&widgets, &state, &tx, &shared, &win);
    callbacks_file::setup_rerun_callback(&widgets, &state, &tx, &shared);
    callbacks_ui::setup_parameter_callbacks(&widgets, &state, &shared);
    callbacks_ui::setup_display_callbacks(&widgets, &state);
    gradient_editor::setup_gradient_editor(&widgets, &state);
    callbacks_ui::setup_playback_callbacks(&widgets, &state);
    callbacks_ui::setup_misc_callbacks(&widgets, &state, &win);
    callbacks_draw::setup_draw_callbacks(&widgets, &state);
    let (x_scroll_gen, y_scroll_gen) = callbacks_nav::setup_scrollbar_callbacks(&widgets, &state);
    callbacks_nav::setup_zoom_callbacks(&widgets, &state);
    callbacks_nav::setup_snap_to_view(&widgets, &state);
    callbacks_nav::setup_spacebar_handler(&mut win, &widgets);
    // Per-widget spacebar guards MUST be last — they set handle() on widgets,
    // which would be overwritten if any later setup also calls handle().
    callbacks_nav::setup_spacebar_guards(&widgets);

    // ── Sync UI widgets to saved settings ──────────────────────────────────
    // Layout hardcodes default values (e.g. "8192" for segment size). After
    // loading the real settings into AppState, push the values into the widgets
    // so the UI matches state from the start.
    {
        let st = state.borrow();
        widgets
            .input_seg_size
            .clone()
            .set_value(&st.fft_params.window_length.to_string());
        let preset_idx = match st.fft_params.window_length {
            256 => 0,
            512 => 1,
            1024 => 2,
            2048 => 3,
            4096 => 4,
            8192 => 5,
            16384 => 6,
            32768 => 7,
            65536 => 8,
            _ => 9,
        };
        widgets.seg_preset_choice.clone().set_value(preset_idx);
        widgets
            .slider_overlap
            .clone()
            .set_value(st.fft_params.overlap_percent as f64);
    }

    // ── Start the 16ms poll loop (worker messages, scrollbar sync, transport) ──
    poll_loop::start_poll_loop(&state, &widgets, &shared, &tx, rx, x_scroll_gen, y_scroll_gen, &win);

    win.show();
    app.run().unwrap();
}

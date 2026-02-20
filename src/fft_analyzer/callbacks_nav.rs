use std::rc::Rc;
use std::cell::{Cell, RefCell};

use fltk::{
    app,
    enums::{Event, Key, Shortcut},
    menu::MenuFlag,
    prelude::*,
    window::Window,
};

use crate::app_state::AppState;
use crate::data::TimeUnit;
use crate::layout::Widgets;
use crate::validation::{attach_float_validation_with_recompute, attach_uint_validation_with_recompute};

// ═══════════════════════════════════════════════════════════════════════════
//  MENU CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_menu_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let mut menu = widgets.menu.clone();

    {
        let mut btn_open = widgets.btn_open.clone();
        menu.add("&File/Open Audio\t", Shortcut::Ctrl | 'o', MenuFlag::Normal,
            move |_| { btn_open.do_callback(); });
    }
    {
        let mut btn_save_fft = widgets.btn_save_fft.clone();
        menu.add("&File/Save FFT Data\t", Shortcut::Ctrl | 's', MenuFlag::Normal,
            move |_| { btn_save_fft.do_callback(); });
    }
    {
        let mut btn_load_fft = widgets.btn_load_fft.clone();
        menu.add("&File/Load FFT Data\t", Shortcut::Ctrl | 'l', MenuFlag::Normal,
            move |_| { btn_load_fft.do_callback(); });
    }
    {
        let mut btn_save_wav = widgets.btn_save_wav.clone();
        menu.add("&File/Export WAV\t", Shortcut::Ctrl | 'e', MenuFlag::Normal,
            move |_| { btn_save_wav.do_callback(); });
    }
    menu.add("&File/Quit\t", Shortcut::Ctrl | 'q', MenuFlag::Normal,
        move |_| { app::quit(); });

    {
        let mut btn_rerun = widgets.btn_rerun.clone();
        menu.add("&Analysis/Recompute FFT\t", Shortcut::None, MenuFlag::Normal,
            move |_| { btn_rerun.do_callback(); });
    }

    {
        let state_c = state.clone();
        let mut spec_display_c = widgets.spec_display.clone();
        let mut freq_axis_c = widgets.freq_axis.clone();
        let mut time_axis_c = widgets.time_axis.clone();
        let mut waveform_c = widgets.waveform_display.clone();
        menu.add("&Display/Reset Zoom\t", Shortcut::None, MenuFlag::Normal,
            move |_| {
                let mut st = state_c.borrow_mut();
                st.view.reset_zoom();
                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
                drop(st);
                spec_display_c.redraw();
                waveform_c.redraw();
                freq_axis_c.redraw();
                time_axis_c.redraw();
            });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  SCROLLBAR CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

/// Sets up X and Y scrollbar callbacks. Returns generation counters used by
/// the poll loop to avoid fighting user drags.
pub fn setup_scrollbar_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) -> (Rc<Cell<u64>>, Rc<Cell<u64>>) {
    let x_scroll_gen = Rc::new(Cell::new(0u64));
    let y_scroll_gen = Rc::new(Cell::new(0u64));

    // X scrollbar: controls time panning (viewport only, no effect on processing)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();
        let mut time_axis = widgets.time_axis.clone();
        let x_scroll_gen = x_scroll_gen.clone();

        let mut x_scroll = widgets.x_scroll.clone();
        x_scroll.set_minimum(0.0);
        x_scroll.set_maximum(10000.0);
        x_scroll.set_slider_size(1.0);
        x_scroll.set_value(0.0);

        x_scroll.set_callback(move |s| {
            x_scroll_gen.set(x_scroll_gen.get() + 1);
            let Ok(mut st) = state.try_borrow_mut() else { return; };
            let data_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
            if data_range <= 0.0 { return; }

            let vis_range = st.view.visible_time_range().max(0.001);
            let scroll_frac = (s.value() / s.maximum()).clamp(0.0, 1.0);
            let start = st.view.data_time_min_sec + scroll_frac * (data_range - vis_range).max(0.0);
            st.view.time_min_sec = start.max(st.view.data_time_min_sec);
            st.view.time_max_sec = (start + vis_range).min(st.view.data_time_max_sec);

            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
            time_axis.redraw();
        });
    }

    // Y scrollbar: controls frequency panning (viewport only)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();
        let y_scroll_gen = y_scroll_gen.clone();

        let mut y_scroll = widgets.y_scroll.clone();
        y_scroll.set_minimum(0.0);
        y_scroll.set_maximum(10000.0);
        y_scroll.set_slider_size(1.0);
        y_scroll.set_value(0.0);

        y_scroll.set_callback(move |s| {
            y_scroll_gen.set(y_scroll_gen.get() + 1);
            let Ok(mut st) = state.try_borrow_mut() else { return; };
            let data_max = st.view.data_freq_max_hz;
            let data_min = 1.0_f32;
            let data_range = data_max - data_min;
            if data_range <= 0.0 { return; }

            let vis_range = st.view.visible_freq_range().max(1.0);
            let scroll_frac = 1.0 - (s.value() as f32 / s.maximum() as f32).clamp(0.0, 1.0);  // inverted for vertical
            let start = data_min + scroll_frac * (data_range - vis_range).max(0.0);
            st.view.freq_min_hz = start.max(data_min);
            st.view.freq_max_hz = (start + vis_range).min(data_max);

            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            freq_axis.redraw();
        });
    }

    (x_scroll_gen, y_scroll_gen)
}

// ═══════════════════════════════════════════════════════════════════════════
//  ZOOM BUTTON CALLBACKS
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_zoom_callbacks(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    // Time zoom in (+)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();
        let mut time_axis = widgets.time_axis.clone();

        let mut btn = widgets.btn_time_zoom_in.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let range = st.view.visible_time_range();
            let center = (st.view.time_min_sec + st.view.time_max_sec) / 2.0;
            let new_range = (range / st.time_zoom_factor as f64).max(0.001);
            st.view.time_min_sec = (center - new_range / 2.0).max(st.view.data_time_min_sec);
            st.view.time_max_sec = st.view.time_min_sec + new_range;
            if st.view.time_max_sec > st.view.data_time_max_sec {
                st.view.time_max_sec = st.view.data_time_max_sec;
                st.view.time_min_sec = (st.view.time_max_sec - new_range).max(st.view.data_time_min_sec);
            }
            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
            time_axis.redraw();
        });
    }

    // Time zoom out (-)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();
        let mut time_axis = widgets.time_axis.clone();

        let mut btn = widgets.btn_time_zoom_out.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let range = st.view.visible_time_range();
            let data_range = st.view.data_time_max_sec - st.view.data_time_min_sec;
            let center = (st.view.time_min_sec + st.view.time_max_sec) / 2.0;
            let new_range = (range * st.time_zoom_factor as f64).min(data_range);
            st.view.time_min_sec = (center - new_range / 2.0).max(st.view.data_time_min_sec);
            st.view.time_max_sec = st.view.time_min_sec + new_range;
            if st.view.time_max_sec > st.view.data_time_max_sec {
                st.view.time_max_sec = st.view.data_time_max_sec;
                st.view.time_min_sec = (st.view.time_max_sec - new_range).max(st.view.data_time_min_sec);
            }
            st.spec_renderer.invalidate();
            st.wave_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            waveform_display.redraw();
            time_axis.redraw();
        });
    }

    // Freq zoom in (+)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();

        let mut btn = widgets.btn_freq_zoom_in.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let range = st.view.visible_freq_range();
            let center = (st.view.freq_min_hz + st.view.freq_max_hz) / 2.0;
            let new_range = (range / st.freq_zoom_factor).max(10.0);
            st.view.freq_min_hz = (center - new_range / 2.0).max(1.0);
            st.view.freq_max_hz = (st.view.freq_min_hz + new_range).min(st.view.data_freq_max_hz);
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            freq_axis.redraw();
        });
    }

    // Freq zoom out (-)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut freq_axis = widgets.freq_axis.clone();

        let mut btn = widgets.btn_freq_zoom_out.clone();
        btn.set_callback(move |_| {
            let mut st = state.borrow_mut();
            let range = st.view.visible_freq_range();
            let new_range = (range * st.freq_zoom_factor).min(st.view.data_freq_max_hz - 1.0);
            let center = (st.view.freq_min_hz + st.view.freq_max_hz) / 2.0;
            st.view.freq_min_hz = (center - new_range / 2.0).max(1.0);
            st.view.freq_max_hz = (st.view.freq_min_hz + new_range).min(st.view.data_freq_max_hz);
            st.spec_renderer.invalidate();
            drop(st);
            spec_display.redraw();
            freq_axis.redraw();
        });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  SNAP TO VIEW
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_snap_to_view(
    widgets: &Widgets,
    state: &Rc<RefCell<AppState>>,
) {
    let state = state.clone();
    let mut input_start = widgets.input_start.clone();
    let mut input_stop = widgets.input_stop.clone();
    let mut input_recon_freq_min = widgets.input_recon_freq_min.clone();
    let mut input_recon_freq_max = widgets.input_recon_freq_max.clone();
    let mut btn_rerun = widgets.btn_rerun.clone();

    let mut btn_snap_to_view = widgets.btn_snap_to_view.clone();
    btn_snap_to_view.set_callback(move |_| {
        {
            let mut st = state.borrow_mut();
            // Copy viewport time to processing time (always store as samples)
            let sr = st.fft_params.sample_rate as f64;
            st.fft_params.start_sample = (st.view.time_min_sec * sr).round() as usize;
            st.fft_params.stop_sample = (st.view.time_max_sec * sr).round() as usize;
            // Display based on current time unit
            match st.fft_params.time_unit {
                TimeUnit::Seconds => {
                    input_start.set_value(&format!("{:.5}", st.fft_params.start_seconds()));
                    input_stop.set_value(&format!("{:.5}", st.fft_params.stop_seconds()));
                }
                TimeUnit::Samples => {
                    input_start.set_value(&st.fft_params.start_sample.to_string());
                    input_stop.set_value(&st.fft_params.stop_sample.to_string());
                }
            }
            // Copy viewport freq to reconstruction freq
            st.view.recon_freq_min_hz = st.view.freq_min_hz;
            st.view.recon_freq_max_hz = st.view.freq_max_hz;
            input_recon_freq_min.set_value(&format!("{:.0}", st.view.freq_min_hz));
            input_recon_freq_max.set_value(&format!("{:.0}", st.view.freq_max_hz));
        }
        // Trigger recompute
        btn_rerun.do_callback();
    });
}

// ═══════════════════════════════════════════════════════════════════════════
//  SPACEBAR HANDLER
// ═══════════════════════════════════════════════════════════════════════════

pub fn setup_spacebar_handler(
    win: &mut Window,
    widgets: &Widgets,
) {
    let mut btn_rerun = widgets.btn_rerun.clone();
    win.handle(move |_, event| {
        let is_space = app::event_key() == Key::from_char(' ');
        if !is_space { return false; }

        match event {
            // Consume KeyDown to prevent space from reaching any focused widget
            // (buttons, dropdowns, text inputs). This is the primary guard.
            Event::KeyDown => true,

            // Trigger recompute on KeyUp (not KeyDown to avoid double-fire)
            Event::KeyUp => {
                btn_rerun.do_callback();
                true
            }

            // VNC/remote desktop may send space as a Shortcut event
            Event::Shortcut => true,

            _ => false,
        }
    });
}

// ═══════════════════════════════════════════════════════════════════════════
//  PER-WIDGET SPACEBAR GUARDS
// ═══════════════════════════════════════════════════════════════════════════

/// Block spacebar from activating any interactive widget (buttons, choices,
/// checkbuttons, sliders, scrollbars). Space is a global recompute shortcut
/// and must never trigger widget-specific behavior (opening dropdowns,
/// toggling checkboxes, activating buttons, etc.).
///
/// EXCEPTION: The top-level menu bar (File, Analyze, Display) is not guarded.
///
/// Text inputs, scrub_slider, and gradient_preview are handled separately
/// (their existing handle() callbacks include space blocking).
///
/// Must be called AFTER all other callback setups so it doesn't get overwritten.
pub fn setup_spacebar_guards(widgets: &Widgets) {
    // Macro to attach a spacebar-blocking handle() to any widget.
    // On KeyDown/Shortcut: consume (blocks widget activation).
    // On KeyUp: trigger recompute via btn_rerun, then consume.
    macro_rules! block_space {
        ($widget:expr, $btn_rerun:expr) => {{
            let mut btn = $btn_rerun.clone();
            let mut w = $widget;
            w.handle(move |_, event| {
                if app::event_key() == Key::from_char(' ') {
                    match event {
                        Event::KeyDown | Event::Shortcut => true,
                        Event::KeyUp => { btn.do_callback(); true }
                        _ => false,
                    }
                } else {
                    false
                }
            });
        }};
    }

    let btn_rerun = widgets.btn_rerun.clone();

    // ── Buttons ──
    // Like Choice widgets, FLTK buttons process space internally before
    // handle() can intercept. clear_visible_focus() on all buttons prevents
    // them from receiving keyboard focus so space never reaches them.
    block_space!(widgets.btn_open.clone(), btn_rerun);
    block_space!(widgets.btn_save_fft.clone(), btn_rerun);
    block_space!(widgets.btn_load_fft.clone(), btn_rerun);
    block_space!(widgets.btn_save_wav.clone(), btn_rerun);
    block_space!(widgets.btn_time_unit.clone(), btn_rerun);
    block_space!(widgets.btn_seg_minus.clone(), btn_rerun);
    block_space!(widgets.btn_seg_plus.clone(), btn_rerun);
    block_space!(widgets.btn_rerun.clone(), btn_rerun);
    block_space!(widgets.btn_snap_to_view.clone(), btn_rerun);
    block_space!(widgets.btn_home.clone(), btn_rerun);
    block_space!(widgets.btn_save_defaults.clone(), btn_rerun);
    block_space!(widgets.btn_play.clone(), btn_rerun);
    block_space!(widgets.btn_pause.clone(), btn_rerun);
    block_space!(widgets.btn_stop.clone(), btn_rerun);
    block_space!(widgets.btn_freq_zoom_in.clone(), btn_rerun);
    block_space!(widgets.btn_freq_zoom_out.clone(), btn_rerun);
    block_space!(widgets.btn_time_zoom_in.clone(), btn_rerun);
    block_space!(widgets.btn_time_zoom_out.clone(), btn_rerun);
    widgets.btn_open.clone().clear_visible_focus();
    widgets.btn_save_fft.clone().clear_visible_focus();
    widgets.btn_load_fft.clone().clear_visible_focus();
    widgets.btn_save_wav.clone().clear_visible_focus();
    widgets.btn_time_unit.clone().clear_visible_focus();
    widgets.btn_seg_minus.clone().clear_visible_focus();
    widgets.btn_seg_plus.clone().clear_visible_focus();
    widgets.btn_rerun.clone().clear_visible_focus();
    widgets.btn_snap_to_view.clone().clear_visible_focus();
    widgets.btn_home.clone().clear_visible_focus();
    widgets.btn_save_defaults.clone().clear_visible_focus();
    widgets.btn_play.clone().clear_visible_focus();
    widgets.btn_pause.clone().clear_visible_focus();
    widgets.btn_stop.clone().clear_visible_focus();
    widgets.btn_freq_zoom_in.clone().clear_visible_focus();
    widgets.btn_freq_zoom_out.clone().clear_visible_focus();
    widgets.btn_time_zoom_in.clone().clear_visible_focus();
    widgets.btn_time_zoom_out.clone().clear_visible_focus();

    // ── Choice dropdowns ──
    // FLTK Choice widgets don't reliably honour handle() for space key —
    // their internal menu system processes keyboard events through a separate
    // path. The reliable fix is clear_visible_focus() which prevents them
    // from ever receiving keyboard focus. They still work fully via mouse.
    // The window-level handler catches the freed-up space event for recompute.
    block_space!(widgets.seg_preset_choice.clone(), btn_rerun);
    block_space!(widgets.window_type_choice.clone(), btn_rerun);
    block_space!(widgets.zero_pad_choice.clone(), btn_rerun);
    block_space!(widgets.colormap_choice.clone(), btn_rerun);
    block_space!(widgets.repeat_choice.clone(), btn_rerun);
    widgets.seg_preset_choice.clone().clear_visible_focus();
    widgets.window_type_choice.clone().clear_visible_focus();
    widgets.zero_pad_choice.clone().clear_visible_focus();
    widgets.colormap_choice.clone().clear_visible_focus();
    widgets.repeat_choice.clone().clear_visible_focus();

    // ── CheckButtons ──
    block_space!(widgets.check_center.clone(), btn_rerun);
    block_space!(widgets.btn_tooltips.clone(), btn_rerun);
    block_space!(widgets.check_lock_active.clone(), btn_rerun);
    widgets.check_center.clone().clear_visible_focus();
    widgets.btn_tooltips.clone().clear_visible_focus();
    widgets.check_lock_active.clone().clear_visible_focus();

    // ── Sliders ──
    block_space!(widgets.slider_overlap.clone(), btn_rerun);
    block_space!(widgets.slider_scale.clone(), btn_rerun);
    block_space!(widgets.slider_threshold.clone(), btn_rerun);
    block_space!(widgets.slider_ceiling.clone(), btn_rerun);
    block_space!(widgets.slider_brightness.clone(), btn_rerun);
    block_space!(widgets.slider_gamma.clone(), btn_rerun);
    widgets.slider_overlap.clone().clear_visible_focus();
    widgets.slider_scale.clone().clear_visible_focus();
    widgets.slider_threshold.clone().clear_visible_focus();
    widgets.slider_ceiling.clone().clear_visible_focus();
    widgets.slider_brightness.clone().clear_visible_focus();
    widgets.slider_gamma.clone().clear_visible_focus();

    // ── Scrollbars ──
    block_space!(widgets.x_scroll.clone(), btn_rerun);
    block_space!(widgets.y_scroll.clone(), btn_rerun);
    widgets.x_scroll.clone().clear_visible_focus();
    widgets.y_scroll.clone().clear_visible_focus();

    // ── Text inputs ──
    // Re-attach validation handlers with recompute trigger.
    // This REPLACES the plain validation handlers set in layout.rs,
    // adding btn_rerun.do_callback() on space KeyUp so recompute fires.
    attach_float_validation_with_recompute(&mut widgets.input_start.clone(), &btn_rerun);
    attach_float_validation_with_recompute(&mut widgets.input_stop.clone(), &btn_rerun);
    attach_uint_validation_with_recompute(&mut widgets.input_seg_size.clone(), &btn_rerun);
    attach_float_validation_with_recompute(&mut widgets.input_kaiser_beta.clone(), &btn_rerun);
    attach_uint_validation_with_recompute(&mut widgets.input_freq_count.clone(), &btn_rerun);
    attach_float_validation_with_recompute(&mut widgets.input_recon_freq_min.clone(), &btn_rerun);
    attach_float_validation_with_recompute(&mut widgets.input_recon_freq_max.clone(), &btn_rerun);
}

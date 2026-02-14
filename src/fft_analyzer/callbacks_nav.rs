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
        menu.add("&Display/Reset Zoom\t", Shortcut::None, MenuFlag::Normal,
            move |_| {
                let mut st = state_c.borrow_mut();
                st.view.reset_zoom();
                st.spec_renderer.invalidate();
                st.wave_renderer.invalidate();
                drop(st);
                spec_display_c.redraw();
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
        });
    }

    // Y scrollbar: controls frequency panning (viewport only)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
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
        });
    }

    // Time zoom out (-)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut waveform_display = widgets.waveform_display.clone();

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
        });
    }

    // Freq zoom in (+)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();

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
        });
    }

    // Freq zoom out (-)
    {
        let state = state.clone();
        let mut spec_display = widgets.spec_display.clone();

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
            // Copy viewport time to processing time
            match st.fft_params.time_unit {
                TimeUnit::Seconds => {
                    st.fft_params.start_time = st.view.time_min_sec;
                    st.fft_params.stop_time = st.view.time_max_sec;
                    input_start.set_value(&format!("{:.5}", st.view.time_min_sec));
                    input_stop.set_value(&format!("{:.5}", st.view.time_max_sec));
                }
                TimeUnit::Samples => {
                    let sr = st.fft_params.sample_rate as f64;
                    st.fft_params.start_time = (st.view.time_min_sec * sr).round();
                    st.fft_params.stop_time = (st.view.time_max_sec * sr).round();
                    input_start.set_value(&format!("{}", st.fft_params.start_time as u64));
                    input_stop.set_value(&format!("{}", st.fft_params.stop_time as u64));
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
        if event == Event::KeyUp && app::event_key() == Key::from_char(' ') {
            println!("Space bar press detected");
            btn_rerun.do_callback();
        }
        false
    });
}

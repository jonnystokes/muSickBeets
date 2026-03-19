#[macro_use]
#[allow(dead_code)]
#[path = "../fft_analyzer/debug_flags.rs"]
mod debug_flags;

#[allow(dead_code)]
#[path = "../fft_analyzer/ui/theme.rs"]
mod theme;
#[allow(dead_code)]
#[path = "../fft_analyzer/ui/tooltips.rs"]
mod tooltips;

mod frame_file;
mod layout;
mod synth;

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use fltk::{
    app, dialog, draw,
    enums::{Align, Color, Font},
    prelude::*,
};

use crate::frame_file::FrameFile;
use crate::layout::Widgets;
use crate::synth::SynthPlayer;

struct AppState {
    frame: Option<FrameFile>,
    preview_samples: Vec<f32>,
    synth: SynthPlayer,
}

fn generate_preview(state: &Rc<RefCell<AppState>>) {
    let preview = state.borrow().synth.preview_samples(2048);
    state.borrow_mut().preview_samples = preview;
}

fn update_info(widgets: &Widgets, frame: &FrameFile) {
    widgets.info_output.clone().set_value(&format!(
        "Frame file loaded\n\nSample rate: {} Hz\nFFT size: {}\nFrame index: {}\nFrame time: {:.5}s\nActive bins: {}\nMax freq: {:.1} Hz",
        frame.sample_rate,
        frame.fft_size,
        frame.frame_index,
        frame.frame_time_seconds,
        frame.active_bin_count,
        frame.max_frequency_hz(),
    ));
}

fn setup_waveform_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.waveform_display.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_PANEL));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());

        let st = state.borrow();
        if st.preview_samples.is_empty() {
            draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
            draw::set_font(Font::Helvetica, 14);
            draw::draw_text2(
                "Waveform preview",
                f.x(),
                f.y(),
                f.w(),
                f.h(),
                Align::Center,
            );
            return;
        }

        let mid_y = f.y() + f.h() / 2;
        draw::set_draw_color(theme::color(theme::BORDER));
        draw::draw_line(f.x(), mid_y, f.x() + f.w(), mid_y);

        draw::set_draw_color(theme::color(theme::ACCENT_GREEN));
        let samples = &st.preview_samples;
        for px in 0..f.w().max(1) {
            let idx = ((px as f32 / f.w().max(1) as f32) * (samples.len().saturating_sub(1)) as f32)
                as usize;
            let v = samples[idx].clamp(-1.0, 1.0);
            let y = mid_y - (v * (f.h() as f32 * 0.42)) as i32;
            draw::draw_point(f.x() + px, y);
        }
    });
}

fn spectrogram_color(v: f32) -> Color {
    let v = v.clamp(0.0, 1.0);
    if v < 0.33 {
        Color::from_rgb(42, 42, 58)
    } else if v < 0.66 {
        Color::from_rgb(137, 180, 250)
    } else {
        Color::from_rgb(249, 226, 175)
    }
}

fn setup_spectrogram_draw(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let state = state.clone();
    let mut w = widgets.spectrogram_display.clone();
    w.draw(move |f| {
        draw::set_draw_color(theme::color(theme::BG_PANEL));
        draw::draw_rectf(f.x(), f.y(), f.w(), f.h());

        let st = state.borrow();
        let Some(frame) = st.frame.as_ref() else {
            draw::set_draw_color(theme::color(theme::TEXT_DISABLED));
            draw::set_font(Font::Helvetica, 14);
            draw::draw_text2(
                "Single-frame spectrogram",
                f.x(),
                f.y(),
                f.w(),
                f.h(),
                Align::Center,
            );
            return;
        };

        let max_mag = frame
            .bins
            .iter()
            .map(|b| b.magnitude)
            .fold(1e-9_f32, f32::max);
        let max_freq = frame.max_frequency_hz().max(1.0);

        for py in 0..f.h().max(1) {
            let t = 1.0 - py as f32 / f.h().max(1) as f32;
            let freq = t * max_freq;

            let mut nearest_mag = 0.0_f32;
            let mut best_df = f32::MAX;
            for bin in &frame.bins {
                let df = (bin.frequency_hz - freq).abs();
                if df < best_df {
                    best_df = df;
                    nearest_mag = bin.magnitude;
                }
            }

            let normalized = (nearest_mag / max_mag).sqrt();
            draw::set_draw_color(spectrogram_color(normalized));
            draw::draw_line(f.x(), f.y() + py, f.x() + f.w(), f.y() + py);
        }
    });
}

fn open_frame_file(state: &Rc<RefCell<AppState>>, widgets: &Widgets) -> Result<()> {
    let mut chooser = dialog::NativeFileChooser::new(dialog::NativeFileChooserType::BrowseFile);
    chooser.set_filter("*.fftframe");
    chooser.show();
    let path = chooser.filename();
    if path.as_os_str().is_empty() {
        return Ok(());
    }

    let frame = FrameFile::from_path(&path)?;
    app_log!(
        "noise_maker",
        "Loaded frame {:?}: bins={} sr={} fft={} frame_index={} time={:.5}s",
        path,
        frame.active_bin_count,
        frame.sample_rate,
        frame.fft_size,
        frame.frame_index,
        frame.frame_time_seconds
    );

    {
        let mut st = state.borrow_mut();
        st.synth.load_frame(&frame)?;
        st.frame = Some(frame.clone());
    }

    generate_preview(state);
    update_info(widgets, &frame);
    widgets.status_bar.clone().set_label(&format!(
        "Loaded frame {} | {} bins | {:.1} Hz max",
        frame.frame_index,
        frame.active_bin_count,
        frame.max_frequency_hz()
    ));
    widgets.btn_play.clone().activate();
    widgets.btn_pause.clone().activate();
    widgets.btn_stop.clone().activate();
    widgets.waveform_display.clone().redraw();
    widgets.spectrogram_display.clone().redraw();
    Ok(())
}

fn main() {
    app::set_option(app::Option::FnfcUsesGtk, false);
    app::set_option(app::Option::FnfcUsesZenity, false);
    unsafe {
        std::env::set_var("GIO_USE_VFS", "local");
        std::env::set_var("GIO_USE_VOLUME_MONITOR", "unix");
        std::env::set_var("GVFS_REMOTE_VOLUME_MONITOR_IGNORE", "1");
    }

    let app = app::App::default();
    theme::apply_dark_theme();
    app::set_visual(fltk::enums::Mode::Rgb8).ok();

    let (mut win, widgets) = layout::build_ui();
    let _tooltip_mgr = tooltips::TooltipManager::new();

    let state = Rc::new(RefCell::new(AppState {
        frame: None,
        preview_samples: Vec::new(),
        synth: SynthPlayer::new(),
    }));

    setup_waveform_draw(&widgets, &state);
    setup_spectrogram_draw(&widgets, &state);

    {
        let state = state.clone();
        let widgets_c = Widgets {
            btn_open_frame: widgets.btn_open_frame.clone(),
            slider_gain: widgets.slider_gain.clone(),
            lbl_gain_value: widgets.lbl_gain_value.clone(),
            info_output: widgets.info_output.clone(),
            waveform_display: widgets.waveform_display.clone(),
            spectrogram_display: widgets.spectrogram_display.clone(),
            btn_play: widgets.btn_play.clone(),
            btn_pause: widgets.btn_pause.clone(),
            btn_stop: widgets.btn_stop.clone(),
            status_bar: widgets.status_bar.clone(),
        };
        let mut btn = widgets.btn_open_frame.clone();
        btn.set_callback(move |_| {
            if let Err(err) = open_frame_file(&state, &widgets_c) {
                app_log!("noise_maker", "Open error: {}", err);
                dialog::alert_default(&format!("Open failed: {}", err));
            }
        });
    }

    {
        let state = state.clone();
        let mut slider = widgets.slider_gain.clone();
        let mut lbl = widgets.lbl_gain_value.clone();
        let mut wave = widgets.waveform_display.clone();
        slider.set_callback(move |s| {
            let gain = s.value() as f32;
            state.borrow_mut().synth.set_gain(gain);
            generate_preview(&state);
            lbl.set_label(&format!("{:.2}x", gain));
            wave.redraw();
        });
    }

    {
        let state = state.clone();
        let mut status = widgets.status_bar.clone();
        let mut btn = widgets.btn_play.clone();
        btn.set_callback(move |_| {
            if state.borrow().synth.has_frame() {
                state.borrow_mut().synth.play();
                status.set_label("Playing continuous frame synthesis");
            }
        });
    }
    {
        let state = state.clone();
        let mut status = widgets.status_bar.clone();
        let mut btn = widgets.btn_pause.clone();
        btn.set_callback(move |_| {
            state.borrow_mut().synth.pause();
            status.set_label("Paused");
        });
    }
    {
        let state = state.clone();
        let mut status = widgets.status_bar.clone();
        let mut btn = widgets.btn_stop.clone();
        btn.set_callback(move |_| {
            state.borrow_mut().synth.stop();
            status.set_label("Stopped and phase reset");
        });
    }

    win.show();
    app.run().unwrap();
}

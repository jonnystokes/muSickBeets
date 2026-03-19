use fltk::{
    button::Button,
    enums::{Align, FrameType},
    frame::Frame,
    group::Flex,
    output::MultilineOutput,
    prelude::*,
    valuator::HorNiceSlider,
    window::Window,
};

use crate::theme;
use crate::tooltips::set_tooltip;

pub struct Widgets {
    pub btn_open_frame: Button,
    pub slider_gain: HorNiceSlider,
    pub lbl_gain_value: Frame,
    pub info_output: MultilineOutput,
    pub waveform_display: Frame,
    pub spectrogram_display: Frame,
    pub btn_play: Button,
    pub btn_pause: Button,
    pub btn_stop: Button,
    pub status_bar: Frame,
}

pub fn build_ui() -> (Window, Widgets) {
    const WIN_W: i32 = 1200;
    const WIN_H: i32 = 900;
    const LEFT_W: i32 = 270;

    let mut win = Window::default()
        .with_size(WIN_W, WIN_H)
        .with_label("muSickBeets - noise_maker");
    win.set_color(theme::color(theme::BG_DARK));

    let mut root = Flex::default_fill().row();
    root.set_margin(8);
    root.set_pad(8);

    let mut left = Flex::default().column();
    left.set_pad(6);
    left.set_color(theme::color(theme::BG_PANEL));
    root.fixed(&left, LEFT_W);

    let mut lbl_file = Frame::default().with_label("FILE");
    lbl_file.set_label_color(theme::section_header_color());
    lbl_file.set_label_size(11);
    lbl_file.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_file, 18);

    let mut btn_open_frame = Button::default().with_label("Open FFT Frame");
    btn_open_frame.set_color(theme::color(theme::BG_WIDGET));
    btn_open_frame.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(
        &mut btn_open_frame,
        "Load a .fftframe export from fft_analyzer.",
    );
    left.fixed(&btn_open_frame, 30);

    let mut sep1 = Frame::default();
    sep1.set_frame(FrameType::FlatBox);
    sep1.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep1, 1);

    let mut lbl_settings = Frame::default().with_label("SETTINGS");
    lbl_settings.set_label_color(theme::section_header_color());
    lbl_settings.set_label_size(11);
    lbl_settings.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_settings, 18);

    let mut gain_label = Frame::default().with_label("Output Gain");
    gain_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    gain_label.set_label_size(11);
    gain_label.set_align(Align::Inside | Align::Left);
    left.fixed(&gain_label, 18);

    let mut slider_gain = HorNiceSlider::default();
    slider_gain.set_range(0.0, 2.0);
    slider_gain.set_step(0.01, 1);
    slider_gain.set_value(1.0);
    slider_gain.set_color(theme::color(theme::BG_WIDGET));
    slider_gain.set_selection_color(theme::color(theme::ACCENT_BLUE));
    set_tooltip(&mut slider_gain, "Master gain for live additive playback.");
    left.fixed(&slider_gain, 24);

    let mut lbl_gain_value = Frame::default().with_label("1.00x");
    lbl_gain_value.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_gain_value.set_label_size(11);
    lbl_gain_value.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_gain_value, 16);

    let mut sep2 = Frame::default();
    sep2.set_frame(FrameType::FlatBox);
    sep2.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep2, 1);

    let mut lbl_info = Frame::default().with_label("INFO");
    lbl_info.set_label_color(theme::section_header_color());
    lbl_info.set_label_size(11);
    lbl_info.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_info, 18);

    let mut info_output = MultilineOutput::default();
    info_output.set_color(theme::color(theme::BG_WIDGET));
    info_output.set_text_color(theme::color(theme::TEXT_PRIMARY));
    info_output.set_text_size(11);
    info_output.set_wrap(true);
    info_output.set_value("Load an FFT frame file to begin.");

    Frame::default();
    left.end();

    let mut right = Flex::default().column();
    right.set_pad(8);

    let mut waveform_display = Frame::default();
    waveform_display.set_frame(FrameType::FlatBox);
    waveform_display.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&waveform_display, 230);

    let mut spectrogram_display = Frame::default();
    spectrogram_display.set_frame(FrameType::FlatBox);
    spectrogram_display.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&spectrogram_display, 320);

    let mut transport = Flex::default().row();
    transport.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&transport, 34);

    Frame::default();
    let mut btn_play = Button::default().with_label("@>");
    btn_play.set_color(theme::color(theme::BG_WIDGET));
    btn_play.set_label_color(theme::color(theme::ACCENT_GREEN));
    btn_play.deactivate();
    transport.fixed(&btn_play, 42);

    let mut btn_pause = Button::default().with_label("@||");
    btn_pause.set_color(theme::color(theme::BG_WIDGET));
    btn_pause.set_label_color(theme::color(theme::ACCENT_YELLOW));
    btn_pause.deactivate();
    transport.fixed(&btn_pause, 42);

    let mut btn_stop = Button::default().with_label("@square");
    btn_stop.set_color(theme::color(theme::BG_WIDGET));
    btn_stop.set_label_color(theme::color(theme::ACCENT_RED));
    btn_stop.deactivate();
    transport.fixed(&btn_stop, 42);
    Frame::default();
    transport.end();

    let mut status_bar = Frame::default().with_label("Ready | Load a frame file");
    status_bar.set_frame(FrameType::FlatBox);
    status_bar.set_color(theme::color(theme::BG_PANEL));
    status_bar.set_label_color(theme::color(theme::TEXT_SECONDARY));
    status_bar.set_label_size(11);
    status_bar.set_align(Align::Inside | Align::Left);
    right.fixed(&status_bar, 24);

    right.end();
    root.end();
    win.end();
    win.resizable(&root);

    (
        win,
        Widgets {
            btn_open_frame,
            slider_gain,
            lbl_gain_value,
            info_output,
            waveform_display,
            spectrogram_display,
            btn_play,
            btn_pause,
            btn_stop,
            status_bar,
        },
    )
}

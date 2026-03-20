use fltk::{
    button::Button,
    enums::{Align, FrameType},
    frame::Frame,
    group::{Flex, Scroll},
    menu::{Choice, MenuBar},
    output::MultilineOutput,
    prelude::*,
    valuator::{HorNiceSlider, Scrollbar, ScrollbarType},
    window::Window,
};

use crate::theme;
use crate::tooltips::set_tooltip;

pub const WIN_W: i32 = 1200;
pub const WIN_H: i32 = 980;
const MENU_H: i32 = 25;
const STATUS_H: i32 = 42;
const SIDEBAR_W: i32 = 250;
const SIDEBAR_INNER_W: i32 = 230;
const SIDEBAR_INNER_H: i32 = 1500;
pub const WAVE_AMP_AXIS_W: i32 = 56;
pub const SPEC_FREQ_AXIS_W: i32 = 56;
pub const RIGHT_SCROLL_W: i32 = 20;

pub struct Widgets {
    pub root: Flex,
    pub menu: MenuBar,
    pub msg_bar: Frame,
    pub btn_key: Button,

    pub btn_open_frame: Button,
    pub btn_save_audio_gene: Button,
    pub btn_load_audio_gene: Button,
    pub btn_export_audio: Button,
    pub btn_save_defaults: Button,

    pub slider_audio_gain: HorNiceSlider,
    pub lbl_audio_gain_value: Frame,
    pub btn_auto_gain: Button,
    pub engine_choice: Choice,
    pub window_choice: Choice,
    pub slider_overlap: HorNiceSlider,
    pub lbl_overlap_value: Frame,

    pub slider_visual_gain: HorNiceSlider,
    pub lbl_visual_gain_value: Frame,
    pub btn_wave_max: Button,

    pub btn_wave_zoom_in: Button,
    pub btn_wave_zoom_out: Button,
    pub btn_amp_zoom_in: Button,
    pub btn_amp_zoom_out: Button,
    pub btn_spec_freq_zoom_in: Button,
    pub btn_spec_freq_zoom_out: Button,
    pub btn_home: Button,
    pub spec_freq_scroll: Scrollbar,

    pub info_output: MultilineOutput,
    pub waveform_display: Frame,
    pub wave_amp_axis: Frame,
    pub wave_time_axis: Frame,
    pub spectrogram_display: Frame,
    pub spec_freq_axis: Frame,
    pub cursor_readout: Frame,
    pub view_readout: Frame,
    pub btn_play: Button,
    pub btn_pause: Button,
    pub btn_stop: Button,
    pub status_bar: MultilineOutput,
}

pub fn build_ui() -> (Window, Widgets) {
    let mut win = Window::new(50, 50, WIN_W, WIN_H, "muSickBeets - noise_maker");
    win.make_resizable(true);
    win.set_color(theme::color(theme::BG_DARK));

    let mut menu_row = Flex::default().with_size(WIN_W, MENU_H).row();
    menu_row.set_pad(0);

    let mut menu = MenuBar::default();
    menu.set_color(theme::color(theme::BG_PANEL));
    menu.set_text_color(theme::color(theme::TEXT_PRIMARY));
    menu.set_text_size(12);
    menu_row.fixed(&menu, 180);

    let mut sep = Frame::default();
    sep.set_frame(FrameType::FlatBox);
    sep.set_color(theme::color(theme::SEPARATOR));
    menu_row.fixed(&sep, 1);

    let mut msg_bar = Frame::default();
    msg_bar.set_frame(FrameType::FlatBox);
    msg_bar.set_color(theme::color(theme::BG_PANEL));
    msg_bar.set_label_color(theme::color(theme::TEXT_SECONDARY));
    msg_bar.set_label_size(11);
    msg_bar.set_align(Align::Inside | Align::Left);

    let mut btn_key = Button::default().with_label("Key");
    btn_key.set_color(theme::color(theme::BG_WIDGET));
    btn_key.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(
        &mut btn_key,
        "Show all keyboard shortcuts used by the program.",
    );
    menu_row.fixed(&btn_key, 52);
    menu_row.end();

    let mut root = Flex::default()
        .with_pos(0, MENU_H)
        .with_size(WIN_W, WIN_H - MENU_H - STATUS_H)
        .row();

    let mut left_scroll = Scroll::default();
    left_scroll.set_color(theme::color(theme::BG_PANEL));
    root.fixed(&left_scroll, SIDEBAR_W);

    let mut left = Flex::default()
        .with_size(SIDEBAR_INNER_W, SIDEBAR_INNER_H)
        .column();
    left.set_margin(5);
    left.set_pad(2);

    let mut lbl_file = Frame::default().with_label("FILE");
    lbl_file.set_label_color(theme::section_header_color());
    lbl_file.set_label_size(11);
    lbl_file.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_file, 18);

    let mut btn_open_frame = Button::default().with_label("Open Frame / Project");
    btn_open_frame.set_color(theme::color(theme::BG_WIDGET));
    btn_open_frame.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(
        &mut btn_open_frame,
        "Open a frame file or audio_gene project.",
    );
    left.fixed(&btn_open_frame, 28);

    let mut btn_save_audio_gene = Button::default().with_label("Save audio_gene");
    btn_save_audio_gene.set_color(theme::color(theme::BG_WIDGET));
    btn_save_audio_gene.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_save_audio_gene.deactivate();
    left.fixed(&btn_save_audio_gene, 28);

    let mut btn_load_audio_gene = Button::default().with_label("Load audio_gene");
    btn_load_audio_gene.set_color(theme::color(theme::BG_WIDGET));
    btn_load_audio_gene.set_label_color(theme::color(theme::TEXT_PRIMARY));
    left.fixed(&btn_load_audio_gene, 28);

    let mut btn_export_audio = Button::default().with_label("Export Audio");
    btn_export_audio.set_color(theme::color(theme::BG_WIDGET));
    btn_export_audio.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_export_audio.deactivate();
    left.fixed(&btn_export_audio, 28);

    let mut btn_save_defaults = Button::default().with_label("Save Defaults");
    btn_save_defaults.set_color(theme::color(theme::BG_WIDGET));
    btn_save_defaults.set_label_color(theme::color(theme::TEXT_PRIMARY));
    left.fixed(&btn_save_defaults, 28);

    let mut sep1 = Frame::default();
    sep1.set_frame(FrameType::FlatBox);
    sep1.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep1, 1);

    let mut lbl_playback = Frame::default().with_label("PLAYBACK");
    lbl_playback.set_label_color(theme::section_header_color());
    lbl_playback.set_label_size(11);
    lbl_playback.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_playback, 18);

    let mut audio_gain_label = Frame::default().with_label("Audio Gain");
    audio_gain_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    audio_gain_label.set_label_size(11);
    audio_gain_label.set_align(Align::Inside | Align::Left);
    left.fixed(&audio_gain_label, 18);

    let mut slider_audio_gain = HorNiceSlider::default();
    slider_audio_gain.set_range(0.0, 8.0);
    slider_audio_gain.set_step(0.01, 1);
    slider_audio_gain.set_value(1.0);
    slider_audio_gain.set_color(theme::color(theme::BG_WIDGET));
    slider_audio_gain.set_selection_color(theme::color(theme::ACCENT_BLUE));
    left.fixed(&slider_audio_gain, 24);

    let mut lbl_audio_gain_value = Frame::default().with_label("1.00x");
    lbl_audio_gain_value.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_audio_gain_value.set_align(Align::Inside | Align::Right);
    lbl_audio_gain_value.set_label_size(11);
    left.fixed(&lbl_audio_gain_value, 16);

    let mut btn_auto_gain = Button::default().with_label("Auto Gain");
    btn_auto_gain.set_color(theme::color(theme::BG_WIDGET));
    btn_auto_gain.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_auto_gain.deactivate();
    left.fixed(&btn_auto_gain, 26);

    let mut engine_label = Frame::default().with_label("Engine");
    engine_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    engine_label.set_label_size(11);
    engine_label.set_align(Align::Inside | Align::Left);
    left.fixed(&engine_label, 18);

    let mut engine_choice = Choice::default();
    engine_choice.add_choice("Osc Bank");
    engine_choice.add_choice("Frame OLA");
    engine_choice.set_value(0);
    engine_choice.set_color(theme::color(theme::BG_WIDGET));
    engine_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    left.fixed(&engine_choice, 24);

    let mut window_label = Frame::default().with_label("Window");
    window_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    window_label.set_label_size(11);
    window_label.set_align(Align::Inside | Align::Left);
    left.fixed(&window_label, 18);

    let mut window_choice = Choice::default();
    window_choice.add_choice("Rectangular");
    window_choice.add_choice("Hann");
    window_choice.add_choice("Hamming");
    window_choice.add_choice("Blackman");
    window_choice.add_choice("Kaiser");
    window_choice.set_value(1);
    window_choice.set_color(theme::color(theme::BG_WIDGET));
    window_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    left.fixed(&window_choice, 24);

    let mut overlap_label = Frame::default().with_label("Overlap");
    overlap_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    overlap_label.set_label_size(11);
    overlap_label.set_align(Align::Inside | Align::Left);
    left.fixed(&overlap_label, 18);

    let mut slider_overlap = HorNiceSlider::default();
    slider_overlap.set_range(0.0, 95.0);
    slider_overlap.set_step(1.0, 1);
    slider_overlap.set_value(0.0);
    slider_overlap.set_color(theme::color(theme::BG_WIDGET));
    slider_overlap.set_selection_color(theme::color(theme::ACCENT_BLUE));
    left.fixed(&slider_overlap, 24);

    let mut lbl_overlap_value = Frame::default().with_label("0%");
    lbl_overlap_value.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_overlap_value.set_align(Align::Inside | Align::Right);
    lbl_overlap_value.set_label_size(11);
    left.fixed(&lbl_overlap_value, 16);

    let mut sep2 = Frame::default();
    sep2.set_frame(FrameType::FlatBox);
    sep2.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep2, 1);

    let mut lbl_display = Frame::default().with_label("DISPLAY");
    lbl_display.set_label_color(theme::section_header_color());
    lbl_display.set_label_size(11);
    lbl_display.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_display, 18);

    let mut visual_gain_label = Frame::default().with_label("Wave Visual Gain");
    visual_gain_label.set_label_color(theme::color(theme::TEXT_SECONDARY));
    visual_gain_label.set_label_size(11);
    visual_gain_label.set_align(Align::Inside | Align::Left);
    left.fixed(&visual_gain_label, 18);

    let mut slider_visual_gain = HorNiceSlider::default();
    slider_visual_gain.set_range(0.1, 20.0);
    slider_visual_gain.set_step(0.01, 1);
    slider_visual_gain.set_value(1.0);
    slider_visual_gain.set_color(theme::color(theme::BG_WIDGET));
    slider_visual_gain.set_selection_color(theme::color(theme::ACCENT_BLUE));
    left.fixed(&slider_visual_gain, 24);

    let mut lbl_visual_gain_value = Frame::default().with_label("1.00x");
    lbl_visual_gain_value.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_visual_gain_value.set_align(Align::Inside | Align::Right);
    lbl_visual_gain_value.set_label_size(11);
    left.fixed(&lbl_visual_gain_value, 16);

    let mut btn_wave_max = Button::default().with_label("Max Visible");
    btn_wave_max.set_color(theme::color(theme::BG_WIDGET));
    btn_wave_max.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_wave_max.deactivate();
    left.fixed(&btn_wave_max, 26);

    let mut sep3 = Frame::default();
    sep3.set_frame(FrameType::FlatBox);
    sep3.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep3, 1);

    let mut lbl_nav = Frame::default().with_label("NAVIGATION");
    lbl_nav.set_label_color(theme::section_header_color());
    lbl_nav.set_label_size(11);
    lbl_nav.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_nav, 18);

    let mut btn_wave_zoom_in = Button::default().with_label("Wave Time +");
    btn_wave_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_wave_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_wave_zoom_in.deactivate();
    left.fixed(&btn_wave_zoom_in, 26);
    let mut btn_wave_zoom_out = Button::default().with_label("Wave Time -");
    btn_wave_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_wave_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_wave_zoom_out.deactivate();
    left.fixed(&btn_wave_zoom_out, 26);
    let mut btn_amp_zoom_in = Button::default().with_label("Amp +");
    btn_amp_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_amp_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_amp_zoom_in.deactivate();
    left.fixed(&btn_amp_zoom_in, 26);
    let mut btn_amp_zoom_out = Button::default().with_label("Amp -");
    btn_amp_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_amp_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_amp_zoom_out.deactivate();
    left.fixed(&btn_amp_zoom_out, 26);
    let mut btn_spec_freq_zoom_in = Button::default().with_label("Freq +");
    btn_spec_freq_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_spec_freq_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_spec_freq_zoom_in.deactivate();
    left.fixed(&btn_spec_freq_zoom_in, 26);
    let mut btn_spec_freq_zoom_out = Button::default().with_label("Freq -");
    btn_spec_freq_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_spec_freq_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_spec_freq_zoom_out.deactivate();
    left.fixed(&btn_spec_freq_zoom_out, 26);
    let mut btn_home = Button::default().with_label("Home");
    btn_home.set_color(theme::color(theme::BG_WIDGET));
    btn_home.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_home.deactivate();
    left.fixed(&btn_home, 26);

    let mut sep4 = Frame::default();
    sep4.set_frame(FrameType::FlatBox);
    sep4.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep4, 1);

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
    info_output.set_value("Load a frame file or audio_gene to begin.");

    Frame::default();
    left.end();
    left_scroll.end();

    let mut right = Flex::default().column();
    right.set_margin(2);
    right.set_pad(2);

    let mut waveform_row = Flex::default().row();
    right.fixed(&waveform_row, 220);
    let mut wave_amp_axis = Frame::default();
    wave_amp_axis.set_frame(FrameType::FlatBox);
    wave_amp_axis.set_color(theme::color(theme::BG_DARK));
    waveform_row.fixed(&wave_amp_axis, WAVE_AMP_AXIS_W);
    let mut waveform_display = Frame::default();
    waveform_display.set_frame(FrameType::FlatBox);
    waveform_display.set_color(theme::color(theme::BG_DARK));
    waveform_row.end();

    let mut wave_time_axis = Frame::default();
    wave_time_axis.set_frame(FrameType::FlatBox);
    wave_time_axis.set_color(theme::color(theme::BG_DARK));
    right.fixed(&wave_time_axis, 22);

    let mut spectrogram_row = Flex::default().row();
    right.fixed(&spectrogram_row, 320);
    let mut spec_freq_axis = Frame::default();
    spec_freq_axis.set_frame(FrameType::FlatBox);
    spec_freq_axis.set_color(theme::color(theme::BG_DARK));
    spectrogram_row.fixed(&spec_freq_axis, SPEC_FREQ_AXIS_W);
    let mut spectrogram_display = Frame::default();
    spectrogram_display.set_frame(FrameType::FlatBox);
    spectrogram_display.set_color(theme::color(theme::BG_DARK));
    let mut spec_freq_scroll = Scrollbar::default();
    spec_freq_scroll.set_type(ScrollbarType::Vertical);
    spec_freq_scroll.set_color(theme::color(theme::BG_WIDGET));
    spec_freq_scroll.set_selection_color(theme::accent_color());
    spectrogram_row.fixed(&spec_freq_scroll, RIGHT_SCROLL_W);
    spectrogram_row.end();

    let mut transport = Flex::default().row();
    transport.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&transport, 34);
    let mut cursor_readout = Frame::default();
    cursor_readout.set_label_color(theme::color(theme::TEXT_SECONDARY));
    cursor_readout.set_label_size(10);
    cursor_readout.set_align(Align::Inside | Align::Right);
    transport.fixed(&cursor_readout, 300);
    let mut view_readout = Frame::default();
    view_readout.set_label_color(theme::color(theme::TEXT_SECONDARY));
    view_readout.set_label_size(10);
    view_readout.set_align(Align::Inside | Align::Right);
    transport.fixed(&view_readout, 230);
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

    let mut status_bar = MultilineOutput::default();
    status_bar.set_frame(FrameType::FlatBox);
    status_bar.set_color(theme::color(theme::BG_PANEL));
    status_bar.set_text_color(theme::color(theme::TEXT_SECONDARY));
    status_bar.set_text_size(11);
    status_bar.set_value("Ready | Load a frame or audio_gene to begin");
    right.fixed(&status_bar, 42);

    right.end();
    root.end();
    win.end();
    win.resizable(&root);

    (
        win,
        Widgets {
            root,
            menu,
            msg_bar,
            btn_key,
            btn_open_frame,
            btn_save_audio_gene,
            btn_load_audio_gene,
            btn_export_audio,
            btn_save_defaults,
            slider_audio_gain,
            lbl_audio_gain_value,
            btn_auto_gain,
            engine_choice,
            window_choice,
            slider_overlap,
            lbl_overlap_value,
            slider_visual_gain,
            lbl_visual_gain_value,
            btn_wave_max,
            btn_wave_zoom_in,
            btn_wave_zoom_out,
            btn_amp_zoom_in,
            btn_amp_zoom_out,
            btn_spec_freq_zoom_in,
            btn_spec_freq_zoom_out,
            btn_home,
            spec_freq_scroll,
            info_output,
            waveform_display,
            wave_amp_axis,
            wave_time_axis,
            spectrogram_display,
            spec_freq_axis,
            cursor_readout,
            view_readout,
            btn_play,
            btn_pause,
            btn_stop,
            status_bar,
        },
    )
}

use fltk::{
    button::Button,
    enums::{Align, FrameType},
    frame::Frame,
    group::Flex,
    input::{FloatInput, Input},
    menu::{Choice, MenuBar},
    output::MultilineOutput,
    prelude::*,
    valuator::HorNiceSlider,
    widget::Widget,
    window::Window,
};

use crate::ui::theme;
use crate::ui::tooltips::set_tooltip;

// ─── Window Layout Constants ────────────────────────────────────────────────────
pub const WIN_W: i32 = 1200;
pub const WIN_H: i32 = 1555;
const MENU_H: i32 = 25;
const STATUS_H: i32 = 25;
const STATUS_FFT_MIN_H: i32 = 0;
pub const STATUS_FFT_OFFSET: i32 = 0;
const SIDEBAR_W: i32 = 215;
const SIDEBAR_INNER_W: i32 = 200;
const SIDEBAR_INNER_H: i32 = 1800;
pub const SPEC_LEFT_GUTTER_W: i32 = 50;
pub const SPEC_RIGHT_GUTTER_W: i32 = 20;

// ─── Widgets struct ─────────────────────────────────────────────────────────────
// Holds cloneable handles to every widget that callbacks need to access.

pub struct Widgets {
    pub root: Flex,
    pub menu: MenuBar,
    pub btn_key: Button,
    pub btn_open: Button,
    pub btn_save_fft: Button,
    pub btn_load_fft: Button,
    pub btn_save_wav: Button,
    pub btn_time_unit: Button,
    pub input_start: FloatInput,
    pub input_stop: FloatInput,
    pub input_seg_size: Input,
    pub seg_preset_choice: Choice,
    pub slider_overlap: HorNiceSlider,
    pub lbl_overlap_val: Frame,
    pub lbl_hop_info: Frame,
    pub input_segments_per_active: Input,
    pub input_bins_per_segment: Input,
    pub window_type_choice: Choice,
    pub input_kaiser_beta: FloatInput,
    pub check_center: fltk::button::CheckButton,
    pub zero_pad_choice: Choice,
    pub lbl_resolution_info: MultilineOutput,
    pub btn_rerun: Button,
    pub colormap_choice: Choice,
    pub gradient_preview: Widget,
    pub slider_scale: HorNiceSlider,
    pub lbl_scale_val: Frame,
    pub slider_threshold: HorNiceSlider,
    pub lbl_threshold_val: Frame,
    pub slider_ceiling: HorNiceSlider,
    pub lbl_ceiling_val: Frame,
    pub slider_brightness: HorNiceSlider,
    pub lbl_brightness_val: Frame,
    pub slider_gamma: HorNiceSlider,
    pub lbl_gamma_val: Frame,
    pub input_freq_count: Input,
    pub input_recon_freq_min: FloatInput,
    pub input_recon_freq_max: FloatInput,
    pub btn_freq_max: Button,
    pub input_norm_floor: FloatInput,
    pub lbl_norm_floor_sci: Frame,
    pub btn_snap_to_view: Button,
    pub lbl_info: MultilineOutput,
    pub btn_tooltips: fltk::button::CheckButton,
    pub check_lock_active: fltk::button::CheckButton,
    pub check_render_full_outside_roi: fltk::button::CheckButton,
    pub btn_home: Button,
    pub btn_save_defaults: Button,
    pub spec_display: Widget,
    pub waveform_display: Widget,
    pub freq_axis: Widget,
    pub time_axis: Widget,
    pub btn_freq_zoom_in: Button,
    pub btn_freq_zoom_out: Button,
    pub y_scroll: fltk::valuator::Scrollbar,
    pub btn_time_zoom_in: Button,
    pub btn_time_zoom_out: Button,
    pub x_scroll: fltk::valuator::Scrollbar,
    pub btn_play: Button,
    pub btn_pause: Button,
    pub btn_stop: Button,
    pub btn_mouse_mode_time: Button,
    pub btn_mouse_mode_move: Button,
    pub btn_mouse_mode_zoom: Button,
    pub btn_mouse_mode_roi: Button,
    pub scrub_slider: Widget,
    pub cursor_readout: Frame,
    pub lbl_time: Frame,
    pub repeat_choice: Choice,
    pub status_fft: MultilineOutput,
    pub status_bar: MultilineOutput,
    pub msg_bar: Frame,
}

// ─── Build UI ───────────────────────────────────────────────────────────────────

pub fn build_ui() -> (Window, Widgets) {
    let mut win = Window::new(50, 50, WIN_W, WIN_H, "muSickBeets FFT Analyzer");
    win.make_resizable(true);
    win.set_color(theme::color(theme::BG_DARK));

    // ═══════════════════════════════════════════════════════════════════════════
    //  MENU BAR + MESSAGE BAR (same row)
    // ═══════════════════════════════════════════════════════════════════════════

    let mut menu_row = Flex::default().with_size(WIN_W, MENU_H).row();
    menu_row.set_pad(0);

    let mut menu = MenuBar::default();
    menu.set_color(theme::color(theme::BG_PANEL));
    menu.set_text_color(theme::color(theme::TEXT_PRIMARY));
    menu.set_text_size(12);
    menu_row.fixed(&menu, 180);

    // Separator between menu and message area
    let mut sep = Frame::default();
    sep.set_frame(FrameType::FlatBox);
    sep.set_color(theme::color(theme::SEPARATOR));
    menu_row.fixed(&sep, 1);

    // Message bar — transient feedback (errors, warnings, notices)
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

    // ═══════════════════════════════════════════════════════════════════════════
    //  ROOT LAYOUT (below menu bar)
    // ═══════════════════════════════════════════════════════════════════════════

    let mut root = Flex::default()
        .with_pos(0, MENU_H)
        .with_size(WIN_W, WIN_H - MENU_H - STATUS_H)
        .row();

    // ─── LEFT PANEL (Controls) ─────────────────────────────────────────────────

    let mut left_scroll = fltk::group::Scroll::default();
    left_scroll.set_color(theme::color(theme::BG_PANEL));
    root.fixed(&left_scroll, SIDEBAR_W);

    let mut left = Flex::default()
        .with_size(SIDEBAR_INNER_W, SIDEBAR_INNER_H)
        .column();
    left.set_margin(5);
    left.set_pad(2);

    // Build all sidebar controls via layout_sidebar module
    let sb = crate::layout_sidebar::build_sidebar(&mut left);

    left.end();
    left_scroll.end();

    // ─── RIGHT PANEL (Display area) ────────────────────────────────────────────

    let mut right = Flex::default().column();
    right.set_margin(2);
    right.set_pad(2);

    // ── Waveform strip ──
    // Keep waveform width locked to the spectrogram's drawable width by
    // bracketing it with the same left/right gutters used by spec_row.
    let mut waveform_row = Flex::default().row();
    right.fixed(&waveform_row, 100);

    let mut waveform_left_spacer = Frame::default();
    waveform_left_spacer.set_frame(FrameType::FlatBox);
    waveform_left_spacer.set_color(theme::color(theme::BG_DARK));
    waveform_row.fixed(&waveform_left_spacer, SPEC_LEFT_GUTTER_W);

    let mut waveform_display = Widget::default();
    waveform_display.set_frame(FrameType::FlatBox);
    waveform_display.set_color(theme::color(theme::BG_DARK));

    let mut waveform_right_spacer = Frame::default();
    waveform_right_spacer.set_frame(FrameType::FlatBox);
    waveform_right_spacer.set_color(theme::color(theme::BG_DARK));
    waveform_row.fixed(&waveform_right_spacer, SPEC_RIGHT_GUTTER_W);

    waveform_row.end();

    // ── Spectrogram area (with Y scrollbar) ──
    let mut spec_row = Flex::default().row();

    // Frequency axis label area
    let mut freq_axis = Widget::default();
    freq_axis.set_frame(FrameType::FlatBox);
    freq_axis.set_color(theme::color(theme::BG_DARK));
    spec_row.fixed(&freq_axis, SPEC_LEFT_GUTTER_W);

    // Main spectrogram display
    let mut spec_display = Widget::default();
    spec_display.set_frame(FrameType::FlatBox);
    spec_display.set_color(theme::color(theme::BG_DARK));

    // Y-axis controls (freq zoom +/- and scrollbar)
    let mut freq_zoom_col = Flex::default().column();
    freq_zoom_col.set_pad(1);

    let mut btn_freq_zoom_in = Button::default().with_label("+");
    btn_freq_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_freq_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_freq_zoom_in.set_label_size(12);
    set_tooltip(&mut btn_freq_zoom_in, "Zoom in on frequency axis.");
    freq_zoom_col.fixed(&btn_freq_zoom_in, 20);

    let mut y_scroll = fltk::valuator::Scrollbar::default();
    y_scroll.set_type(fltk::valuator::ScrollbarType::Vertical);
    y_scroll.set_color(theme::color(theme::BG_WIDGET));
    y_scroll.set_selection_color(theme::accent_color());
    set_tooltip(
        &mut y_scroll,
        "Frequency axis pan.\nDrag to scroll up/down.",
    );

    let mut btn_freq_zoom_out = Button::default().with_label("-");
    btn_freq_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_freq_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_freq_zoom_out.set_label_size(12);
    set_tooltip(&mut btn_freq_zoom_out, "Zoom out on frequency axis.");
    freq_zoom_col.fixed(&btn_freq_zoom_out, 20);

    freq_zoom_col.end();
    spec_row.fixed(&freq_zoom_col, SPEC_RIGHT_GUTTER_W);

    spec_row.end();

    // ── Time axis label area ──
    // Keep this full-width; the draw code applies the shared left/right
    // spectrogram gutters internally so labels align with the spectrogram
    // drawable region without double-counting offsets.
    let mut time_axis = Widget::default();
    time_axis.set_frame(FrameType::FlatBox);
    time_axis.set_color(theme::color(theme::BG_DARK));
    right.fixed(&time_axis, 20);

    // ── X-axis controls (time zoom +/- and scrollbar) ──
    let mut time_zoom_row = Flex::default().row();
    time_zoom_row.set_pad(1);

    let mut btn_time_zoom_out = Button::default().with_label("-");
    btn_time_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_time_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_time_zoom_out.set_label_size(12);
    set_tooltip(&mut btn_time_zoom_out, "Zoom out on time axis.");
    time_zoom_row.fixed(&btn_time_zoom_out, 20);

    let mut x_scroll = fltk::valuator::Scrollbar::default();
    x_scroll.set_type(fltk::valuator::ScrollbarType::Horizontal);
    x_scroll.set_color(theme::color(theme::BG_WIDGET));
    x_scroll.set_selection_color(theme::accent_color());
    set_tooltip(
        &mut x_scroll,
        "Time axis pan.\nDrag to scroll left/right.\nMouse wheel on spectrogram to zoom.",
    );

    let mut btn_time_zoom_in = Button::default().with_label("+");
    btn_time_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_time_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_time_zoom_in.set_label_size(12);
    set_tooltip(&mut btn_time_zoom_in, "Zoom in on time axis.");
    time_zoom_row.fixed(&btn_time_zoom_in, 20);

    time_zoom_row.end();
    right.fixed(&time_zoom_row, 20);

    // ── Scrubber row ──
    // Match scrubber width to the spectrogram drawable area so the playback
    // position geometry lines up with the spectrogram, waveform, and time axis.
    let mut scrub_row = Flex::default().row();
    scrub_row.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&scrub_row, 18);

    let mut scrub_left_spacer = Frame::default();
    scrub_left_spacer.set_frame(FrameType::FlatBox);
    scrub_left_spacer.set_color(theme::color(theme::BG_PANEL));
    scrub_row.fixed(&scrub_left_spacer, SPEC_LEFT_GUTTER_W);

    let mut scrub_slider = Widget::default();
    scrub_slider.set_frame(FrameType::FlatBox);
    scrub_slider.set_color(theme::color(theme::BG_WIDGET));
    scrub_slider.deactivate();
    set_tooltip(
        &mut scrub_slider,
        "Playback position scrubber.\nDrag to seek. Audio plays from drag position when in play mode.",
    );

    let mut scrub_right_spacer = Frame::default();
    scrub_right_spacer.set_frame(FrameType::FlatBox);
    scrub_right_spacer.set_color(theme::color(theme::BG_PANEL));
    scrub_row.fixed(&scrub_right_spacer, SPEC_RIGHT_GUTTER_W);

    scrub_row.end();

    // ── Transport controls row (buttons | cursor readout | time | repeat) ──
    let mut transport_row = Flex::default().row();
    transport_row.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&transport_row, 28);

    let mut btn_play = Button::default().with_label("@>");
    btn_play.set_color(theme::color(theme::BG_WIDGET));
    btn_play.set_label_color(theme::color(theme::ACCENT_GREEN));
    btn_play.deactivate();
    set_tooltip(&mut btn_play, "Play audio from current position.");
    transport_row.fixed(&btn_play, 36);

    let mut btn_pause = Button::default().with_label("@||");
    btn_pause.set_color(theme::color(theme::BG_WIDGET));
    btn_pause.set_label_color(theme::color(theme::ACCENT_YELLOW));
    btn_pause.deactivate();
    set_tooltip(&mut btn_pause, "Pause playback at current position.");
    transport_row.fixed(&btn_pause, 36);

    let mut btn_stop = Button::default().with_label("@square");
    btn_stop.set_color(theme::color(theme::BG_WIDGET));
    btn_stop.set_label_color(theme::color(theme::ACCENT_RED));
    btn_stop.deactivate();
    set_tooltip(&mut btn_stop, "Stop playback and reset to start.");
    transport_row.fixed(&btn_stop, 36);

    let mut mode_gap = Frame::default();
    mode_gap.set_frame(FrameType::FlatBox);
    mode_gap.set_color(theme::color(theme::BG_PANEL));
    transport_row.fixed(&mode_gap, 50);

    let mut lbl_mouse_mode = Frame::default().with_label("Mode");
    lbl_mouse_mode.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_mouse_mode.set_label_size(11);
    lbl_mouse_mode.set_align(Align::Inside | Align::Left);
    transport_row.fixed(&lbl_mouse_mode, 36);

    let mut btn_mouse_mode_time = Button::default().with_label("Time");
    btn_mouse_mode_time.set_color(theme::color(theme::ACCENT_BLUE));
    btn_mouse_mode_time.set_label_color(theme::color(theme::BG_DARK));
    btn_mouse_mode_time.deactivate();
    set_tooltip(
        &mut btn_mouse_mode_time,
        "Mouse mode: Time. Click or drag in the spectrogram or waveform to seek playback.",
    );
    transport_row.fixed(&btn_mouse_mode_time, 48);

    let mut btn_mouse_mode_move = Button::default().with_label("Move");
    btn_mouse_mode_move.set_color(theme::color(theme::BG_WIDGET));
    btn_mouse_mode_move.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_mouse_mode_move.deactivate();
    set_tooltip(
        &mut btn_mouse_mode_move,
        "Mouse mode: Move. Drag the spectrogram to pan time and frequency, or drag the waveform to pan time.",
    );
    transport_row.fixed(&btn_mouse_mode_move, 52);

    let mut btn_mouse_mode_zoom = Button::default().with_label("Sel Zoom");
    btn_mouse_mode_zoom.set_color(theme::color(theme::BG_WIDGET));
    btn_mouse_mode_zoom.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_mouse_mode_zoom.deactivate();
    set_tooltip(
        &mut btn_mouse_mode_zoom,
        "Mouse mode: Select Zoom. Drag a box to zoom into that region.",
    );
    transport_row.fixed(&btn_mouse_mode_zoom, 66);

    let mut btn_mouse_mode_roi = Button::default().with_label("ROI Sel");
    btn_mouse_mode_roi.set_color(theme::color(theme::BG_WIDGET));
    btn_mouse_mode_roi.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_mouse_mode_roi.deactivate();
    set_tooltip(
        &mut btn_mouse_mode_roi,
        "Mouse mode: ROI Select. Drag a box to set Start/Stop and Recon Min/Max without recomputing.",
    );
    transport_row.fixed(&btn_mouse_mode_roi, 60);

    // Flexible spacer pushes everything after it to the right
    Frame::default();

    // Cursor readout — shows freq/dB/time under mouse on spectrogram.
    // Takes remaining flex space; text is right-aligned to sit flush
    // against lbl_time.
    let mut cursor_readout = Frame::default();
    cursor_readout.set_label_color(theme::color(theme::TEXT_SECONDARY));
    cursor_readout.set_label_size(10);
    cursor_readout.set_align(Align::Inside | Align::Right);

    let mut lbl_time = Frame::default().with_label("L 0:00.00 / 0:00.00\nG 0:00.00");
    lbl_time.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_time.set_label_size(10);
    lbl_time.set_align(Align::Inside | Align::Right);
    transport_row.fixed(&lbl_time, 170);

    let mut repeat_choice = Choice::default();
    repeat_choice.add_choice("Single");
    repeat_choice.add_choice("Repeat");
    repeat_choice.set_value(0);
    repeat_choice.set_color(theme::color(theme::BG_WIDGET));
    repeat_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    repeat_choice.deactivate();
    set_tooltip(
        &mut repeat_choice,
        "Single: stop at end.\nRepeat: loop continuously.",
    );
    transport_row.fixed(&repeat_choice, 70);

    transport_row.end();

    right.end();
    root.end();

    // ─── FFT STATUS READOUT (auto-expanded height controlled by callback) ───

    let mut status_fft = MultilineOutput::default()
        .with_pos(0, WIN_H - STATUS_H - STATUS_FFT_MIN_H - STATUS_FFT_OFFSET)
        .with_size(WIN_W, STATUS_FFT_MIN_H);
    status_fft.set_color(theme::color(theme::BG_PANEL));
    status_fft.set_text_color(theme::color(theme::TEXT_SECONDARY));
    status_fft.set_text_size(11);
    status_fft.set_wrap(true);
    set_tooltip(
        &mut status_fft,
        "My active time range (the selected portion of the full audio) is divided into overlapping segments; each segment is then transformed into a set of frequency bins – the vertical columns of the spectrogram.",
    );

    // ─── STATUS BAR ───────────────────────────────────────────────────────────

    let mut status_bar = MultilineOutput::default()
        .with_pos(0, WIN_H - STATUS_H)
        .with_size(WIN_W, STATUS_H);
    status_bar.set_value("Ready  |  Load an audio file to begin");
    status_bar.set_color(theme::color(theme::BG_PANEL));
    status_bar.set_text_color(theme::color(theme::TEXT_SECONDARY));
    status_bar.set_text_size(11);

    win.end();

    // Make the window resize properly
    win.resizable(&root);

    let widgets = Widgets {
        root,
        menu,
        btn_key,
        btn_open: sb.btn_open,
        btn_save_fft: sb.btn_save_fft,
        btn_load_fft: sb.btn_load_fft,
        btn_save_wav: sb.btn_save_wav,
        btn_time_unit: sb.btn_time_unit,
        input_start: sb.input_start,
        input_stop: sb.input_stop,
        input_seg_size: sb.input_seg_size,
        seg_preset_choice: sb.seg_preset_choice,
        slider_overlap: sb.slider_overlap,
        lbl_overlap_val: sb.lbl_overlap_val,
        lbl_hop_info: sb.lbl_hop_info,
        input_segments_per_active: sb.input_segments_per_active,
        input_bins_per_segment: sb.input_bins_per_segment,
        window_type_choice: sb.window_type_choice,
        input_kaiser_beta: sb.input_kaiser_beta,
        check_center: sb.check_center,
        zero_pad_choice: sb.zero_pad_choice,
        lbl_resolution_info: sb.lbl_resolution_info,
        btn_rerun: sb.btn_rerun,
        colormap_choice: sb.colormap_choice,
        gradient_preview: sb.gradient_preview,
        slider_scale: sb.slider_scale,
        lbl_scale_val: sb.lbl_scale_val,
        slider_threshold: sb.slider_threshold,
        lbl_threshold_val: sb.lbl_threshold_val,
        slider_ceiling: sb.slider_ceiling,
        lbl_ceiling_val: sb.lbl_ceiling_val,
        slider_brightness: sb.slider_brightness,
        lbl_brightness_val: sb.lbl_brightness_val,
        slider_gamma: sb.slider_gamma,
        lbl_gamma_val: sb.lbl_gamma_val,
        input_freq_count: sb.input_freq_count,
        input_recon_freq_min: sb.input_recon_freq_min,
        input_recon_freq_max: sb.input_recon_freq_max,
        btn_freq_max: sb.btn_freq_max,
        input_norm_floor: sb.input_norm_floor,
        lbl_norm_floor_sci: sb.lbl_norm_floor_sci,
        btn_snap_to_view: sb.btn_snap_to_view,
        lbl_info: sb.lbl_info,
        btn_tooltips: sb.btn_tooltips,
        check_lock_active: sb.check_lock_active,
        check_render_full_outside_roi: sb.check_render_full_outside_roi,
        btn_home: sb.btn_home,
        btn_save_defaults: sb.btn_save_defaults,
        spec_display,
        waveform_display,
        freq_axis,
        time_axis,
        btn_freq_zoom_in,
        btn_freq_zoom_out,
        y_scroll,
        btn_time_zoom_in,
        btn_time_zoom_out,
        x_scroll,
        btn_play,
        btn_pause,
        btn_stop,
        btn_mouse_mode_time,
        btn_mouse_mode_move,
        btn_mouse_mode_zoom,
        btn_mouse_mode_roi,
        scrub_slider,
        cursor_readout,
        lbl_time,
        repeat_choice,
        status_fft,
        status_bar,
        msg_bar,
    };

    (win, widgets)
}

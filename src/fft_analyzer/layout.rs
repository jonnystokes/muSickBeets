use fltk::{
    button::Button,
    enums::{Align, FrameType},
    frame::Frame,
    group::Flex,
    input::{Input, FloatInput},
    menu::{Choice, MenuBar},
    prelude::*,
    valuator::{HorNiceSlider, HorSlider},
    widget::Widget,
    window::Window,
};

use crate::data::ColormapId;
use crate::ui::theme;
use crate::ui::tooltips::set_tooltip;
use crate::validation::{attach_float_validation, attach_uint_validation};

// ─── Window Layout Constants ────────────────────────────────────────────────────
pub const WIN_W: i32 = 1200;
pub const WIN_H: i32 = 1200;
const MENU_H: i32 = 25;
const STATUS_H: i32 = 25;
const SIDEBAR_W: i32 = 215;
const SIDEBAR_INNER_W: i32 = 200;
const SIDEBAR_INNER_H: i32 = 1100;

// ─── Widgets struct ─────────────────────────────────────────────────────────────
// Holds cloneable handles to every widget that callbacks need to access.

pub struct Widgets {
    pub menu: MenuBar,
    pub btn_open: Button,
    pub btn_save_fft: Button,
    pub btn_load_fft: Button,
    pub btn_save_wav: Button,
    pub btn_time_unit: Button,
    pub input_start: FloatInput,
    pub input_stop: FloatInput,
    pub btn_seg_minus: Button,
    pub btn_seg_plus: Button,
    pub lbl_seg_value: Frame,
    pub slider_overlap: HorNiceSlider,
    pub lbl_overlap_val: Frame,
    pub window_type_choice: Choice,
    pub input_kaiser_beta: FloatInput,
    pub check_center: fltk::button::CheckButton,
    pub btn_rerun: Button,
    pub colormap_choice: Choice,
    pub scale_choice: Choice,
    pub slider_threshold: HorNiceSlider,
    pub lbl_threshold_val: Frame,
    pub slider_brightness: HorNiceSlider,
    pub lbl_brightness_val: Frame,
    pub slider_gamma: HorNiceSlider,
    pub lbl_gamma_val: Frame,
    pub input_freq_count: Input,
    pub input_recon_freq_min: FloatInput,
    pub input_recon_freq_max: FloatInput,
    pub btn_snap_to_view: Button,
    pub lbl_info: Frame,
    pub btn_tooltips: fltk::button::CheckButton,
    pub check_lock_active: fltk::button::CheckButton,
    pub btn_home: Button,
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
    pub scrub_slider: HorSlider,
    pub lbl_time: Frame,
    pub repeat_choice: Choice,
    pub status_bar: Frame,
}

// ─── Build UI ───────────────────────────────────────────────────────────────────

pub fn build_ui() -> (Window, Widgets) {
    let mut win = Window::new(50, 50, WIN_W, WIN_H, "muSickBeets FFT Analyzer");
    win.make_resizable(true);
    win.set_color(theme::color(theme::BG_DARK));

    // ═══════════════════════════════════════════════════════════════════════════
    //  MENU BAR
    // ═══════════════════════════════════════════════════════════════════════════

    let mut menu = MenuBar::default().with_size(WIN_W, MENU_H);
    menu.set_color(theme::color(theme::BG_PANEL));
    menu.set_text_color(theme::color(theme::TEXT_PRIMARY));
    menu.set_text_size(12);

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

    // ── Title ──
    let mut title = Frame::default().with_label("FFT Analyzer");
    title.set_label_size(15);
    title.set_label_color(theme::color(theme::ACCENT_BLUE));
    left.fixed(&title, 28);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: File Operations
    // ════════════════════════════════════════════════════════════════

    let mut lbl_file = Frame::default().with_label("FILE");
    lbl_file.set_label_color(theme::section_header_color());
    lbl_file.set_label_size(11);
    lbl_file.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_file, 18);

    let mut btn_open = Button::default().with_label("Open Audio File");
    btn_open.set_color(theme::color(theme::BG_WIDGET));
    btn_open.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut btn_open, "Open a WAV audio file for analysis.\nSupports 8/16/24/32-bit PCM and float formats.");
    left.fixed(&btn_open, 28);

    let mut btn_save_fft = Button::default().with_label("Save FFT Data");
    btn_save_fft.set_color(theme::color(theme::BG_WIDGET));
    btn_save_fft.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_save_fft.deactivate();
    set_tooltip(&mut btn_save_fft, "Export spectrogram data to CSV.\nRequires FFT data to be computed first.");
    left.fixed(&btn_save_fft, 28);

    let mut btn_load_fft = Button::default().with_label("Load FFT Data");
    btn_load_fft.set_color(theme::color(theme::BG_WIDGET));
    btn_load_fft.set_label_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut btn_load_fft, "Import previously saved FFT data from CSV.");
    left.fixed(&btn_load_fft, 28);

    let mut btn_save_wav = Button::default().with_label("Export WAV");
    btn_save_wav.set_color(theme::color(theme::BG_WIDGET));
    btn_save_wav.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_save_wav.deactivate();
    set_tooltip(&mut btn_save_wav, "Save reconstructed audio as 16-bit WAV.\nReconstruct audio first, then export.");
    left.fixed(&btn_save_wav, 28);

    // Separator
    let mut sep1 = Frame::default();
    sep1.set_frame(FrameType::FlatBox);
    sep1.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep1, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Analysis Parameters
    // ════════════════════════════════════════════════════════════════

    let mut lbl_analysis = Frame::default().with_label("ANALYSIS");
    lbl_analysis.set_label_color(theme::section_header_color());
    lbl_analysis.set_label_size(11);
    lbl_analysis.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_analysis, 18);

    // Time range
    let mut btn_time_unit = Button::default().with_label("Unit: Seconds");
    btn_time_unit.set_color(theme::color(theme::BG_WIDGET));
    btn_time_unit.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_time_unit.set_label_size(11);
    btn_time_unit.deactivate();
    set_tooltip(&mut btn_time_unit, "Toggle between Seconds and Samples.\nClicking converts the start/stop values.");
    left.fixed(&btn_time_unit, 25);

    let mut input_start = FloatInput::default().with_label("Start:");
    input_start.set_value("0");
    input_start.set_color(theme::color(theme::BG_WIDGET));
    input_start.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_start.deactivate();
    set_tooltip(&mut input_start, "Analysis start position.\nFunctional range: 0 to audio duration.\nYou can go outside this range if you want.");
    attach_float_validation(&mut input_start);
    left.fixed(&input_start, 25);

    let mut input_stop = FloatInput::default().with_label("Stop:");
    input_stop.set_value("0");
    input_stop.set_color(theme::color(theme::BG_WIDGET));
    input_stop.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_stop.deactivate();
    set_tooltip(&mut input_stop, "Analysis stop position.\nFunctional range: 0 to audio duration.\nYou can go outside this range if you want.");
    attach_float_validation(&mut input_stop);
    left.fixed(&input_stop, 25);

    // Window length (segments) with +/- buttons
    let mut lbl_wl = Frame::default().with_label("Segment Size:");
    lbl_wl.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_wl.set_label_size(11);
    lbl_wl.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_wl, 16);

    let mut seg_row = Flex::default().row();
    seg_row.set_pad(2);

    let mut btn_seg_minus = Button::default().with_label("-");
    btn_seg_minus.set_color(theme::color(theme::BG_WIDGET));
    btn_seg_minus.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_seg_minus.set_label_size(14);
    btn_seg_minus.deactivate();
    set_tooltip(&mut btn_seg_minus, "Halve the segment size.\nSmaller segments = better time resolution, worse frequency resolution.");
    seg_row.fixed(&btn_seg_minus, 30);

    let mut btn_seg_plus = Button::default().with_label("+");
    btn_seg_plus.set_color(theme::color(theme::BG_WIDGET));
    btn_seg_plus.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_seg_plus.set_label_size(14);
    btn_seg_plus.deactivate();
    set_tooltip(&mut btn_seg_plus, "Double the segment size.\nLarger segments = better frequency resolution, worse time resolution.");
    seg_row.fixed(&btn_seg_plus, 30);

    let mut lbl_seg_value = Frame::default().with_label("2048 smp / 42.67 ms");
    lbl_seg_value.set_label_color(theme::color(theme::TEXT_PRIMARY));
    lbl_seg_value.set_label_size(10);
    lbl_seg_value.set_align(Align::Inside | Align::Left);

    seg_row.end();
    left.fixed(&seg_row, 25);

    // Overlap
    let mut slider_overlap = HorNiceSlider::default();
    slider_overlap.set_minimum(0.0);
    slider_overlap.set_maximum(95.0);
    slider_overlap.set_value(75.0);
    slider_overlap.set_color(theme::color(theme::BG_WIDGET));
    slider_overlap.set_selection_color(theme::accent_color());
    slider_overlap.deactivate();
    set_tooltip(&mut slider_overlap, "Overlap between adjacent FFT windows.\nFunctional range: 0% to 95%.\nHigher = more time frames, smoother spectrogram.\n75% is standard for Hann window.");
    left.fixed(&slider_overlap, 22);

    let mut lbl_overlap_val = Frame::default().with_label("Overlap: 75%");
    lbl_overlap_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_overlap_val.set_label_size(11);
    lbl_overlap_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_overlap_val, 14);

    // Window type
    let mut window_type_choice = Choice::default();
    window_type_choice.add_choice("Hann");
    window_type_choice.add_choice("Hamming");
    window_type_choice.add_choice("Blackman");
    window_type_choice.add_choice("Kaiser");
    window_type_choice.set_value(0);
    window_type_choice.set_color(theme::color(theme::BG_WIDGET));
    window_type_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    window_type_choice.deactivate();
    set_tooltip(&mut window_type_choice, "Windowing function applied to each FFT segment.\nHann: general purpose, good frequency resolution.\nHamming: slightly better sidelobe rejection.\nBlackman: best sidelobe rejection, wider main lobe.\nKaiser: configurable via beta parameter.");
    left.fixed(&window_type_choice, 25);

    let mut input_kaiser_beta = FloatInput::default().with_label("Kaiser B:");
    input_kaiser_beta.set_value("8.6");
    input_kaiser_beta.set_color(theme::color(theme::BG_WIDGET));
    input_kaiser_beta.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_kaiser_beta.deactivate();
    set_tooltip(&mut input_kaiser_beta, "Kaiser window beta parameter.\nFunctional range: 0.0 to 20.0.\nHigher = narrower main lobe, higher sidelobes.\n8.6 approximates a Blackman window.");
    left.fixed(&input_kaiser_beta, 25);

    let mut check_center = fltk::button::CheckButton::default().with_label(" Center/Pad");
    check_center.set_checked(true);
    check_center.set_label_color(theme::color(theme::TEXT_PRIMARY));
    check_center.deactivate();
    set_tooltip(&mut check_center, "Add zero-padding around signal for symmetric analysis.\nRecommended: ON for most use cases.");
    left.fixed(&check_center, 22);

    let mut btn_rerun = Button::default().with_label("Recompute + Rebuild (Space)");
    btn_rerun.set_color(theme::color(theme::ACCENT_BLUE));
    btn_rerun.set_label_color(theme::color(theme::BG_DARK));
    btn_rerun.set_label_size(11);
    btn_rerun.deactivate();
    set_tooltip(&mut btn_rerun, "Rerun FFT + reconstruct audio with current parameters.\nShortcut: Spacebar (works from anywhere).\nAll outputs (spectrogram, waveform, audio) will update.");
    left.fixed(&btn_rerun, 28);

    // Separator
    let mut sep2 = Frame::default();
    sep2.set_frame(FrameType::FlatBox);
    sep2.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep2, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Display
    // ════════════════════════════════════════════════════════════════

    let mut lbl_display = Frame::default().with_label("DISPLAY");
    lbl_display.set_label_color(theme::section_header_color());
    lbl_display.set_label_size(11);
    lbl_display.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_display, 18);

    // Colormap
    let mut colormap_choice = Choice::default();
    for cm in ColormapId::ALL {
        colormap_choice.add_choice(cm.name());
    }
    colormap_choice.set_value(0);
    colormap_choice.set_color(theme::color(theme::BG_WIDGET));
    colormap_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut colormap_choice, "Color scheme for the spectrogram display.\nClassic: blue-cyan-green-yellow-red (rainbow)\nViridis/Magma/Inferno: perceptually uniform scientific colormaps\nGreyscale: black to white\nInverted Grey: white to black (print-friendly)");
    left.fixed(&colormap_choice, 25);

    // Scale toggle
    let mut scale_choice = Choice::default();
    scale_choice.add_choice("Log Frequency");
    scale_choice.add_choice("Linear Frequency");
    scale_choice.set_value(0);
    scale_choice.set_color(theme::color(theme::BG_WIDGET));
    scale_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    set_tooltip(&mut scale_choice, "Frequency axis scaling.\nLog: musical/perceptual spacing (recommended).\n  Octaves get equal visual space.\nLinear: uniform Hz spacing.\n  High frequencies dominate the display.");
    left.fixed(&scale_choice, 25);

    // Threshold
    let mut slider_threshold = HorNiceSlider::default();
    slider_threshold.set_minimum(-120.0);
    slider_threshold.set_maximum(0.0);
    slider_threshold.set_value(-80.0);
    slider_threshold.set_color(theme::color(theme::BG_WIDGET));
    slider_threshold.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_threshold, "Minimum dB level to display.\nFunctional range: -120 dB to 0 dB.\nAnything below this threshold appears as background color.\nLower = show more quiet detail. Higher = focus on loud content.");
    left.fixed(&slider_threshold, 22);

    let mut lbl_threshold_val = Frame::default().with_label("Threshold: -124 dB");
    lbl_threshold_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_threshold_val.set_label_size(11);
    lbl_threshold_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_threshold_val, 14);

    // Brightness
    let mut slider_brightness = HorNiceSlider::default();
    slider_brightness.set_minimum(0.1);
    slider_brightness.set_maximum(3.0);
    slider_brightness.set_value(1.0);
    slider_brightness.set_color(theme::color(theme::BG_WIDGET));
    slider_brightness.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_brightness, "Overall brightness multiplier.\nFunctional range: 0.1 to 3.0.\n1.0 = neutral. Higher = brighter colors for quiet content.");
    left.fixed(&slider_brightness, 22);

    let mut lbl_brightness_val = Frame::default().with_label("Brightness: 1.0");
    lbl_brightness_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_brightness_val.set_label_size(11);
    lbl_brightness_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_brightness_val, 14);

    // Gamma
    let mut slider_gamma = HorNiceSlider::default();
    slider_gamma.set_minimum(0.5);
    slider_gamma.set_maximum(5.0);
    slider_gamma.set_value(2.2);
    slider_gamma.set_color(theme::color(theme::BG_WIDGET));
    slider_gamma.set_selection_color(theme::accent_color());
    set_tooltip(&mut slider_gamma, "Perceptual gamma correction for dB display.\nFunctional range: 0.5 to 5.0.\n2.2 = standard perceptual gamma (recommended).\nHigher = more contrast, quiet content less visible.\nLower = flatter, quiet content more visible.");
    left.fixed(&slider_gamma, 22);

    let mut lbl_gamma_val = Frame::default().with_label("Gamma: 2.2");
    lbl_gamma_val.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_gamma_val.set_label_size(11);
    lbl_gamma_val.set_align(Align::Inside | Align::Right);
    left.fixed(&lbl_gamma_val, 14);

    // Separator
    let mut sep3 = Frame::default();
    sep3.set_frame(FrameType::FlatBox);
    sep3.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep3, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Reconstruction
    // ════════════════════════════════════════════════════════════════

    let mut lbl_recon = Frame::default().with_label("RECONSTRUCTION");
    lbl_recon.set_label_color(theme::section_header_color());
    lbl_recon.set_label_size(11);
    lbl_recon.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_recon, 18);

    // Frequency count
    let mut lbl_fc = Frame::default().with_label("Freq Count:");
    lbl_fc.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_fc.set_label_size(11);
    lbl_fc.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_fc, 16);

    let mut input_freq_count = Input::default();
    input_freq_count.set_value("1025");
    input_freq_count.set_color(theme::color(theme::BG_WIDGET));
    input_freq_count.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_freq_count.deactivate();
    set_tooltip(&mut input_freq_count, "Number of top-magnitude frequency bins to keep per frame.\nFunctional range: 1 to max bins (shown in INFO).\nMax = perfect reconstruction. Lower = simplified/filtered sound.\nAt 1, only the loudest frequency per frame is reconstructed.");
    attach_uint_validation(&mut input_freq_count);
    left.fixed(&input_freq_count, 25);

    // Frequency range
    let mut lbl_freq_min = Frame::default().with_label("Recon Min Freq (Hz):");
    lbl_freq_min.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_freq_min.set_label_size(11);
    lbl_freq_min.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_freq_min, 16);

    let mut input_recon_freq_min = FloatInput::default();
    input_recon_freq_min.set_value("0");
    input_recon_freq_min.set_color(theme::color(theme::BG_WIDGET));
    input_recon_freq_min.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_recon_freq_min.deactivate();
    set_tooltip(&mut input_recon_freq_min, "Minimum frequency for reconstruction.\nFunctional range: 0 to Nyquist.\nBins below this frequency are zeroed out.");
    attach_float_validation(&mut input_recon_freq_min);
    left.fixed(&input_recon_freq_min, 25);

    let mut lbl_freq_max = Frame::default().with_label("Recon Max Freq (Hz):");
    lbl_freq_max.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_freq_max.set_label_size(11);
    lbl_freq_max.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_freq_max, 16);

    let mut input_recon_freq_max = FloatInput::default();
    input_recon_freq_max.set_value("24000");
    input_recon_freq_max.set_color(theme::color(theme::BG_WIDGET));
    input_recon_freq_max.set_text_color(theme::color(theme::TEXT_PRIMARY));
    input_recon_freq_max.deactivate();
    set_tooltip(&mut input_recon_freq_max, "Maximum frequency for reconstruction.\nFunctional range: 0 to Nyquist.\nBins above this frequency are zeroed out.");
    attach_float_validation(&mut input_recon_freq_max);
    left.fixed(&input_recon_freq_max, 25);

    // Snap viewport to processing window
    let mut btn_snap_to_view = Button::default().with_label("Snap to View");
    btn_snap_to_view.set_color(theme::color(theme::BG_WIDGET));
    btn_snap_to_view.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_snap_to_view.set_label_size(11);
    btn_snap_to_view.deactivate();
    set_tooltip(&mut btn_snap_to_view, "Copy current viewport bounds into\nStart/Stop and Freq Min/Max fields.\nThen recompute.");
    left.fixed(&btn_snap_to_view, 25);

    // Separator
    let mut sep4 = Frame::default();
    sep4.set_frame(FrameType::FlatBox);
    sep4.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep4, 1);

    // ════════════════════════════════════════════════════════════════
    //  SECTION: Info Panel (read-only)
    // ════════════════════════════════════════════════════════════════

    let mut lbl_info_header = Frame::default().with_label("INFO");
    lbl_info_header.set_label_color(theme::section_header_color());
    lbl_info_header.set_label_size(11);
    lbl_info_header.set_align(Align::Inside | Align::Left);
    left.fixed(&lbl_info_header, 18);

    let mut lbl_info = Frame::default().with_label("No audio loaded");
    lbl_info.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_info.set_label_size(10);
    lbl_info.set_align(Align::Inside | Align::Left | Align::Top);
    left.fixed(&lbl_info, 110);

    // Separator
    let mut sep5 = Frame::default();
    sep5.set_frame(FrameType::FlatBox);
    sep5.set_color(theme::color(theme::SEPARATOR));
    left.fixed(&sep5, 1);

    // Tooltip toggle
    let mut btn_tooltips = fltk::button::CheckButton::default().with_label(" Show Tooltips");
    btn_tooltips.set_checked(true);
    btn_tooltips.set_label_color(theme::color(theme::TEXT_SECONDARY));
    btn_tooltips.set_label_size(10);
    set_tooltip(&mut btn_tooltips, "Toggle tooltip help bubbles on/off.");
    left.fixed(&btn_tooltips, 22);

    // Lock viewport to active area toggle
    let mut check_lock_active = fltk::button::CheckButton::default().with_label(" Lock to Active");
    check_lock_active.set_checked(false);
    check_lock_active.set_label_color(theme::color(theme::TEXT_SECONDARY));
    check_lock_active.set_label_size(10);
    set_tooltip(&mut check_lock_active, "When checked, viewport auto-snaps to\nthe processing time range on recompute.");
    left.fixed(&check_lock_active, 22);

    // Home button — snap viewport to processing range
    let mut btn_home = Button::default().with_label("Home");
    btn_home.set_color(theme::color(theme::BG_WIDGET));
    btn_home.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_home.set_label_size(11);
    set_tooltip(&mut btn_home, "Snap viewport to the active processing\ntime range (sidebar Start/Stop).");
    left.fixed(&btn_home, 25);

    // Spacer to push everything up
    Frame::default();

    left.end();
    left_scroll.end();

    // ─── RIGHT PANEL (Display area) ────────────────────────────────────────────

    let mut right = Flex::default().column();
    right.set_margin(2);
    right.set_pad(2);

    // ── Waveform strip ──
    let mut waveform_display = Widget::default();
    waveform_display.set_frame(FrameType::FlatBox);
    waveform_display.set_color(theme::color(theme::BG_DARK));
    right.fixed(&waveform_display, 100);

    // ── Spectrogram area (with Y scrollbar) ──
    let mut spec_row = Flex::default().row();

    // Frequency axis label area
    let mut freq_axis = Widget::default();
    freq_axis.set_frame(FrameType::FlatBox);
    freq_axis.set_color(theme::color(theme::BG_DARK));
    spec_row.fixed(&freq_axis, 50);

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
    set_tooltip(&mut y_scroll, "Frequency axis pan.\nDrag to scroll up/down.");

    let mut btn_freq_zoom_out = Button::default().with_label("-");
    btn_freq_zoom_out.set_color(theme::color(theme::BG_WIDGET));
    btn_freq_zoom_out.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_freq_zoom_out.set_label_size(12);
    set_tooltip(&mut btn_freq_zoom_out, "Zoom out on frequency axis.");
    freq_zoom_col.fixed(&btn_freq_zoom_out, 20);

    freq_zoom_col.end();
    spec_row.fixed(&freq_zoom_col, 20);

    spec_row.end();

    // ── Time axis label area ──
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
    set_tooltip(&mut x_scroll, "Time axis pan.\nDrag to scroll left/right.\nMouse wheel on spectrogram to zoom.");

    let mut btn_time_zoom_in = Button::default().with_label("+");
    btn_time_zoom_in.set_color(theme::color(theme::BG_WIDGET));
    btn_time_zoom_in.set_label_color(theme::color(theme::TEXT_PRIMARY));
    btn_time_zoom_in.set_label_size(12);
    set_tooltip(&mut btn_time_zoom_in, "Zoom in on time axis.");
    time_zoom_row.fixed(&btn_time_zoom_in, 20);

    time_zoom_row.end();
    right.fixed(&time_zoom_row, 20);

    // ── Transport bar ──
    let mut transport_row = Flex::default().row();
    transport_row.set_color(theme::color(theme::BG_PANEL));
    right.fixed(&transport_row, 32);

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

    // Scrub slider
    let mut scrub_slider = HorSlider::default();
    scrub_slider.set_minimum(0.0);
    scrub_slider.set_maximum(1.0);
    scrub_slider.set_value(0.0);
    scrub_slider.set_color(theme::color(theme::BG_WIDGET));
    scrub_slider.set_selection_color(theme::color(theme::ACCENT_RED));
    scrub_slider.deactivate();
    set_tooltip(&mut scrub_slider, "Playback position scrubber.\nDrag to seek. Audio plays from drag position when in play mode.");

    let mut lbl_time = Frame::default().with_label("0:00.00 / 0:00.00");
    lbl_time.set_label_color(theme::color(theme::TEXT_SECONDARY));
    lbl_time.set_label_size(11);
    transport_row.fixed(&lbl_time, 120);

    let mut repeat_choice = Choice::default();
    repeat_choice.add_choice("Single");
    repeat_choice.add_choice("Repeat");
    repeat_choice.set_value(0);
    repeat_choice.set_color(theme::color(theme::BG_WIDGET));
    repeat_choice.set_text_color(theme::color(theme::TEXT_PRIMARY));
    repeat_choice.deactivate();
    set_tooltip(&mut repeat_choice, "Single: stop at end.\nRepeat: loop continuously.");
    transport_row.fixed(&repeat_choice, 70);

    transport_row.end();

    right.end();
    root.end();

    // ─── STATUS BAR ───────────────────────────────────────────────────────────

    let mut status_bar = Frame::default()
        .with_pos(0, WIN_H - STATUS_H)
        .with_size(WIN_W, STATUS_H)
        .with_label("Ready | Load an audio file to begin");
    status_bar.set_frame(FrameType::FlatBox);
    status_bar.set_color(theme::color(theme::BG_PANEL));
    status_bar.set_label_color(theme::color(theme::TEXT_SECONDARY));
    status_bar.set_label_size(11);
    status_bar.set_align(Align::Inside | Align::Left);

    win.end();

    // Make the window resize properly
    win.resizable(&root);

    let widgets = Widgets {
        menu,
        btn_open,
        btn_save_fft,
        btn_load_fft,
        btn_save_wav,
        btn_time_unit,
        input_start,
        input_stop,
        btn_seg_minus,
        btn_seg_plus,
        lbl_seg_value,
        slider_overlap,
        lbl_overlap_val,
        window_type_choice,
        input_kaiser_beta,
        check_center,
        btn_rerun,
        colormap_choice,
        scale_choice,
        slider_threshold,
        lbl_threshold_val,
        slider_brightness,
        lbl_brightness_val,
        slider_gamma,
        lbl_gamma_val,
        input_freq_count,
        input_recon_freq_min,
        input_recon_freq_max,
        btn_snap_to_view,
        lbl_info,
        btn_tooltips,
        check_lock_active,
        btn_home,
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
        scrub_slider,
        lbl_time,
        repeat_choice,
        status_bar,
    };

    (win, widgets)
}

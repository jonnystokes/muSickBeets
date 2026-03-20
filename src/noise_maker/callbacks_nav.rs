use std::cell::RefCell;
use std::rc::Rc;

use fltk::{app, dialog, enums::Shortcut, menu::MenuFlag, prelude::*};

use crate::app_state::AppState;
use crate::layout::Widgets;

fn shortcut_key_text() -> &'static str {
    "Keyboard shortcuts\n\nfile and transport\n  Ctrl+O       Open frame or audio_gene\n  Ctrl+S       Save audio_gene\n  Ctrl+L       Load audio_gene\n  Ctrl+E       Export audio\n  Ctrl+Q       Quit the program\n  Space        Play / pause\n  Escape       Close this keys window / active dialogs\n\nwaveform mouse + wheel\n  Wheel            Zoom waveform time\n  Shift + Wheel    Zoom waveform amplitude\n  Alt + Wheel      Pan waveform time\n  Drag             Pan waveform view\n\nspectrogram mouse + wheel\n  Wheel            Zoom spectrogram frequency\n  Alt + Wheel      Pan spectrogram frequency\n\nmenu-only actions\n  Save Defaults    Save current startup defaults\n  Reset Zoom       Restore waveform + spectrogram view defaults"
}

pub fn setup_shortcut_key_button(widgets: &Widgets) {
    let mut btn_key = widgets.btn_key.clone();
    btn_key.set_callback(move |_| {
        dialog::message_title_default("Shortcut Keys");
        dialog::message_default(shortcut_key_text());
    });
}

/// Placeholder for future window-level shortcut handling (spacebar guards, etc.).
pub fn setup_window_shortcuts(_widgets: &Widgets, _state: &Rc<RefCell<AppState>>) {}

pub fn setup_menu_callbacks(widgets: &Widgets, _state: &Rc<RefCell<AppState>>) {
    let mut menu = widgets.menu.clone();

    {
        let mut btn = widgets.btn_open_frame.clone();
        menu.add(
            "&File/Open Frame or Project\t",
            Shortcut::Ctrl | 'o',
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    {
        let mut btn = widgets.btn_save_audio_gene.clone();
        menu.add(
            "&File/Save audio_gene\t",
            Shortcut::Ctrl | 's',
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    {
        let mut btn = widgets.btn_load_audio_gene.clone();
        menu.add(
            "&File/Load audio_gene\t",
            Shortcut::Ctrl | 'l',
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    {
        let mut btn = widgets.btn_export_audio.clone();
        menu.add(
            "&File/Export Audio\t",
            Shortcut::Ctrl | 'e',
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    {
        let mut btn = widgets.btn_save_defaults.clone();
        menu.add(
            "&File/Save Defaults\t",
            Shortcut::None,
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    menu.add(
        "&File/Quit\t",
        Shortcut::Ctrl | 'q',
        MenuFlag::Normal,
        move |_| app::quit(),
    );

    {
        let mut btn = widgets.btn_wave_max.clone();
        menu.add(
            "&Display/Waveform Max Visible\t",
            Shortcut::None,
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
    {
        let mut btn = widgets.btn_home.clone();
        menu.add(
            "&Display/Reset Zoom\t",
            Shortcut::None,
            MenuFlag::Normal,
            move |_| btn.do_callback(),
        );
    }
}

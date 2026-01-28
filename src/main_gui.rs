



use fltk::{
    app,
    button::Button,
    enums::{Align, Color, Font, FrameType},
    frame::Frame,
    group::{Flex, Pack, PackType},
    input::MultilineInput,
    misc::Progress,
    prelude::*,
    valuator::{Slider, SliderType},
    widget::Widget,
    window::Window,
};

use std::cell::Cell;
use std::rc::Rc;

fn main() {
    let a = app::App::default();

    let mut win = Window::new(100, 100, 900, 550, "Audio Tool UI (FLTK, no GPU)");
    win.make_resizable(true);

    // Root layout: left controls, right graphics/text
    let mut root = Flex::default_fill().row();

    // ---------------- Left panel (controls) ----------------
    let mut left = Flex::default().column();
    // FIX: set fixed width from the parent, not left borrowing itself
    root.fixed(&left, 280);

    let mut title = Frame::default().with_label("Controls");
    title.set_label_size(18);
    title.set_align(Align::Inside | Align::Left);

    // Buttons row
    let mut btn_row = Pack::default().with_type(PackType::Horizontal);
    btn_row.set_spacing(8);

    let mut btn_play = Button::default().with_label("Play");
    let mut btn_stop = Button::default().with_label("Stop");
    let mut btn_reset = Button::default().with_label("Reset");
    btn_row.end();

    // Sliders
    let mut s_gain = Slider::default().with_label("Gain");
    s_gain.set_type(SliderType::Horizontal);
    s_gain.set_range(0.0, 1.0);
    s_gain.set_value(0.5);

    let mut s_mix = Slider::default().with_label("Mix");
    s_mix.set_type(SliderType::Horizontal);
    s_mix.set_range(0.0, 1.0);
    s_mix.set_value(0.25);

    // Progress bar
    let mut prog = Progress::default().with_label("Progress");
    prog.set_minimum(0.0);
    prog.set_maximum(100.0);
    prog.set_value(0.0);

    let mut status = Frame::default().with_label("Status: idle");
    status.set_align(Align::Inside | Align::Left);

    left.end();

    // ---------------- Right panel (graphics + text) ----------------
    let mut right = Flex::default().column();

    let mut gfx_label = Frame::default().with_label("Display (waveform/lines)");
    gfx_label.set_align(Align::Inside | Align::Left);

    let mut gfx = Widget::default();
    gfx.set_frame(FrameType::DownBox);

    // FIX: set fixed height from the parent (right)
    right.fixed(&gfx, 300);

    let mut text_label = Frame::default().with_label("Notes / Log (editable, wrapping)");
    text_label.set_align(Align::Inside | Align::Left);

    let mut text = MultilineInput::default();
    text.set_text_font(Font::Courier);
    text.set_text_size(14);
    text.set_wrap(true);
    text.set_value("Type here...\nThis box wraps long lines.\n");

    right.end();
    root.end();
    win.end();

    // ---------------- Callbacks ----------------
    {
        let mut status = status.clone();
        let mut prog = prog.clone();
        btn_play.set_callback(move |_| {
            status.set_label("Status: playing");
            prog.set_value(10.0);
        });
    }
    {
        let mut status = status.clone();
        let mut prog = prog.clone();
        btn_stop.set_callback(move |_| {
            status.set_label("Status: stopped");
            prog.set_value(0.0);
        });
    }
    {
        let mut status = status.clone();
        let mut prog = prog.clone();
        let mut text = text.clone();
        btn_reset.set_callback(move |_| {
            status.set_label("Status: reset");
            prog.set_value(0.0);
            text.set_value("");
        });
    }

    {
        let mut status = status.clone();
        let mut prog = prog.clone();
        let s_mix2 = s_mix.clone();
        s_gain.set_callback(move |s| {
            let g = s.value();
            let m = s_mix2.value();
            status.set_label(&format!("Status: gain={:.2}, mix={:.2}", g, m));
            prog.set_value((g * 100.0).clamp(0.0, 100.0));
        });
    }
    {
        let mut status = status.clone();
        let mut prog = prog.clone();
        let s_gain2 = s_gain.clone();
        s_mix.set_callback(move |s| {
            let m = s.value();
            let g = s_gain2.value();
            status.set_label(&format!("Status: gain={:.2}, mix={:.2}", g, m));
            prog.set_value((m * 100.0).clamp(0.0, 100.0));
        });
    }

    // ---------------- Graphics draw + timer ----------------
    let phase = Rc::new(Cell::new(0.0f32));

    {
        let phase = phase.clone();
        gfx.draw(move |w| {
            use fltk::draw;

            draw::set_draw_color(Color::White);
            draw::draw_rectf(w.x(), w.y(), w.w(), w.h());

            draw::set_draw_color(Color::Black);
            draw::draw_rect(w.x(), w.y(), w.w(), w.h());

            let midy = w.y() + w.h() / 2;
            draw::set_draw_color(Color::from_rgb(180, 180, 180));
            draw::draw_line(w.x(), midy, w.x() + w.w(), midy);

            draw::set_draw_color(Color::Blue);

            let width = w.w().max(2);
            let height = w.h().max(2) as f32;
            let amp = height * 0.35;

            let ph = phase.get();

            let mut last_x = w.x();
            let mut last_y = midy;

            for i in 0..width {
                let x = w.x() + i;
                let t = (i as f32 / width as f32) * 6.28318 * 2.0;
                let y = (midy as f32 + (t + ph).sin() * amp) as i32;
                if i > 0 {
                    draw::draw_line(last_x, last_y, x, y);
                }
                last_x = x;
                last_y = y;
            }
        });
    }

    // Handle-based timer that re-schedules itself
    {
        let mut gfx = gfx.clone();
        let phase = phase.clone();

        let handle_cell: Rc<Cell<app::TimeoutHandle>> = Rc::new(Cell::new(std::ptr::null_mut()));
        let handle_cell2 = handle_cell.clone();

        let h = app::add_timeout3(1.0 / 30.0, move |_h| {
            phase.set(phase.get() + 0.10);
            gfx.redraw();
            app::repeat_timeout3(1.0 / 30.0, handle_cell2.get());
        });

        handle_cell.set(h);
    }

    win.show();
    a.run().unwrap();
}


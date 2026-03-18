use std::cell::RefCell;
use std::rc::Rc;

use fltk::prelude::*;

use crate::app_state::AppState;
use crate::data::{eval_gradient, ColormapId, GradientStop};
use crate::layout::Widgets;

// ═══════════════════════════════════════════════════════════════════════════
//  GRADIENT EDITOR (draw + handle callbacks for the gradient preview widget)
// ═══════════════════════════════════════════════════════════════════════════

/// Internal state for the gradient editor mouse interaction.
struct GradientEditorState {
    selected_stop: Option<usize>,
    dragging: bool,
}

const GRAD_BAR_H: i32 = 20; // height of the gradient bar
const STOP_HANDLE_H: i32 = 10; // height of the triangle handles below

/// Pixel margin from widget left/right edges for the gradient bar
const GRAD_MARGIN: i32 = 4;

pub fn setup_gradient_editor(widgets: &Widgets, state: &Rc<RefCell<AppState>>) {
    let editor_state = Rc::new(RefCell::new(GradientEditorState {
        selected_stop: None,
        dragging: false,
    }));

    // ── Draw callback ──
    {
        let state = state.clone();
        let editor_state = editor_state.clone();
        let mut gradient_preview = widgets.gradient_preview.clone();
        gradient_preview.draw(move |w| {
            use fltk::draw;
            use fltk::enums::{Color, Font};

            let x = w.x();
            let y = w.y();
            let ww = w.w();
            let _wh = w.h();

            // Background
            draw::set_draw_color(Color::from_hex(0x313244));
            draw::draw_rectf(x, y, ww, GRAD_BAR_H + STOP_HANDLE_H);

            let bar_x = x + GRAD_MARGIN;
            let bar_w = ww - GRAD_MARGIN * 2;
            if bar_w <= 0 {
                return;
            }

            // Draw gradient bar pixel by pixel
            let st = state.borrow();
            let is_custom = st.view.colormap == ColormapId::Custom;
            drop(st);

            // Read stops (we need them for drawing; re-borrow for short duration)
            let st = state.borrow();
            let stops_snapshot: Vec<GradientStop> = st.view.custom_gradient.clone();
            drop(st);

            for px in 0..bar_w {
                let t = px as f32 / bar_w.max(1) as f32;
                let (r, g, b) = eval_gradient(&stops_snapshot, t);
                let color = Color::from_rgb(
                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                );
                draw::set_draw_color(color);
                draw::draw_rectf(bar_x + px, y, 1, GRAD_BAR_H);
            }

            // Draw border around bar
            draw::set_draw_color(Color::from_hex(0x6c7086));
            draw::draw_rect(bar_x, y, bar_w, GRAD_BAR_H);

            // Draw stop handles as triangles below the bar
            let es = editor_state.borrow();
            let handle_y = y + GRAD_BAR_H;
            for (i, stop) in stops_snapshot.iter().enumerate() {
                let cx = bar_x + (stop.position * bar_w as f32) as i32;
                let is_selected = es.selected_stop == Some(i) && is_custom;

                // Triangle pointing up
                let half = 4;
                if is_selected {
                    draw::set_draw_color(Color::from_hex(0xffffff));
                } else if is_custom {
                    draw::set_draw_color(Color::from_hex(0xcdd6f4));
                } else {
                    draw::set_draw_color(Color::from_hex(0x6c7086));
                }
                draw::begin_polygon();
                draw::vertex((cx - half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::vertex(cx as f64, handle_y as f64);
                draw::vertex((cx + half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::end_polygon();

                // Outline
                draw::set_draw_color(Color::from_hex(0x1e1e2e));
                draw::begin_line();
                draw::vertex((cx - half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::vertex(cx as f64, handle_y as f64);
                draw::vertex((cx + half) as f64, (handle_y + STOP_HANDLE_H) as f64);
                draw::end_line();
            }
            drop(es);

            // If not in custom mode, draw a subtle overlay label
            if !is_custom {
                draw::set_draw_color(Color::from_hex(0x6c7086));
                draw::set_font(Font::Helvetica, 9);
                let label = "Select 'Custom' to edit";
                let tw = draw::width(label) as i32;
                draw::draw_text(label, bar_x + (bar_w - tw) / 2, y + GRAD_BAR_H / 2 + 3);
            }
        });
    }

    // ── Handle callback (mouse interaction) ──
    {
        let state = state.clone();
        let editor_state = editor_state.clone();
        let mut spec_display = widgets.spec_display.clone();
        let mut gradient_preview_redraw = widgets.gradient_preview.clone();

        let mut gradient_preview = widgets.gradient_preview.clone();
        let mut btn_rerun_grad = widgets.btn_rerun.clone();
        gradient_preview.handle(move |w, ev| {
            // Block spacebar and trigger recompute on KeyUp
            if fltk::app::event_key() == fltk::enums::Key::from_char(' ') {
                return match ev {
                    fltk::enums::Event::KeyDown | fltk::enums::Event::Shortcut => true,
                    fltk::enums::Event::KeyUp => {
                        btn_rerun_grad.do_callback();
                        true
                    }
                    _ => false,
                };
            }
            // Only allow interaction when colormap is Custom
            {
                let st = state.borrow();
                if st.view.colormap != ColormapId::Custom {
                    return false;
                }
            }

            let x = w.x();
            let ww = w.w();
            let bar_x = x + GRAD_MARGIN;
            let bar_w = ww - GRAD_MARGIN * 2;
            if bar_w <= 0 {
                return false;
            }

            match ev {
                fltk::enums::Event::Push => {
                    let mx = fltk::app::event_x();
                    let _my = fltk::app::event_y();
                    let button = fltk::app::event_button();
                    let clicks = fltk::app::event_clicks();

                    let pos_t = ((mx - bar_x) as f32 / bar_w as f32).clamp(0.0, 1.0);

                    // Check if clicking on an existing stop handle
                    let st = state.borrow();
                    let stops = &st.view.custom_gradient;
                    let hit_stop = find_stop_at_x(stops, pos_t, bar_w);
                    drop(st);

                    if button == 3 {
                        // Right-click: delete stop (keep minimum 2)
                        if let Some(idx) = hit_stop {
                            let mut st = state.borrow_mut();
                            if st.view.custom_gradient.len() > 2 {
                                st.view.custom_gradient.remove(idx);
                                st.spec_renderer.invalidate();
                                drop(st);
                                let mut es = editor_state.borrow_mut();
                                es.selected_stop = None;
                                es.dragging = false;
                                drop(es);
                                spec_display.redraw();
                                gradient_preview_redraw.redraw();
                            }
                        }
                        return true;
                    }

                    if let Some(idx) = hit_stop {
                        // Clicked on existing stop
                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = Some(idx);
                        es.dragging = true;

                        // Double-click: open color picker
                        if clicks {
                            es.dragging = false;
                            drop(es);
                            let st = state.borrow();
                            let stop = st.view.custom_gradient[idx];
                            drop(st);

                            let cur_r = (stop.r.clamp(0.0, 1.0) * 255.0) as u8;
                            let cur_g = (stop.g.clamp(0.0, 1.0) * 255.0) as u8;
                            let cur_b = (stop.b.clamp(0.0, 1.0) * 255.0) as u8;
                            let (nr, ng, nb) = fltk::dialog::color_chooser_with_default(
                                "Pick Stop Color",
                                fltk::dialog::ColorMode::Rgb,
                                (cur_r, cur_g, cur_b),
                            );
                            // color_chooser_with_default returns the chosen color
                            // (same as input if cancelled - compare to detect change)
                            if nr != cur_r || ng != cur_g || nb != cur_b {
                                let mut st = state.borrow_mut();
                                st.view.custom_gradient[idx].r = nr as f32 / 255.0;
                                st.view.custom_gradient[idx].g = ng as f32 / 255.0;
                                st.view.custom_gradient[idx].b = nb as f32 / 255.0;
                                st.spec_renderer.invalidate();
                                drop(st);
                                spec_display.redraw();
                            }
                        }
                        gradient_preview_redraw.redraw();
                        return true;
                    }

                    // Clicked on empty space: add new stop with interpolated color
                    {
                        let mut st = state.borrow_mut();
                        let (r, g, b) = eval_gradient(&st.view.custom_gradient, pos_t);
                        let new_stop = GradientStop::new(pos_t, r, g, b);
                        // Insert sorted by position
                        let insert_idx = st
                            .view
                            .custom_gradient
                            .iter()
                            .position(|s| s.position > pos_t)
                            .unwrap_or(st.view.custom_gradient.len());
                        st.view.custom_gradient.insert(insert_idx, new_stop);
                        st.spec_renderer.invalidate();
                        drop(st);

                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = Some(insert_idx);
                        es.dragging = true;
                        drop(es);

                        spec_display.redraw();
                        gradient_preview_redraw.redraw();
                    }
                    true
                }

                fltk::enums::Event::Drag => {
                    let es = editor_state.borrow();
                    if !es.dragging {
                        return false;
                    }
                    let idx = match es.selected_stop {
                        Some(i) => i,
                        None => return false,
                    };
                    drop(es);

                    let mx = fltk::app::event_x();
                    let pos_t = ((mx - bar_x) as f32 / bar_w as f32).clamp(0.0, 1.0);

                    let mut st = state.borrow_mut();
                    if idx < st.view.custom_gradient.len() {
                        st.view.custom_gradient[idx].position = pos_t;
                        // Re-sort stops and update selected index
                        let stop = st.view.custom_gradient[idx];
                        st.view
                            .custom_gradient
                            .sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                        // Find where our stop ended up after sort
                        let new_idx = st.view.custom_gradient.iter().position(|s| {
                            (s.position - stop.position).abs() < 1e-6
                                && (s.r - stop.r).abs() < 1e-6
                                && (s.g - stop.g).abs() < 1e-6
                                && (s.b - stop.b).abs() < 1e-6
                        });
                        drop(st);

                        let mut es = editor_state.borrow_mut();
                        es.selected_stop = new_idx;
                        drop(es);

                        state.borrow_mut().spec_renderer.invalidate();
                        spec_display.redraw();
                        gradient_preview_redraw.redraw();
                    }
                    true
                }

                fltk::enums::Event::Released => {
                    let mut es = editor_state.borrow_mut();
                    es.dragging = false;
                    true
                }

                _ => false,
            }
        });
    }
}

/// Find which stop handle (if any) is near the given normalized position.
/// Returns the index of the nearest stop within a 6-pixel tolerance.
fn find_stop_at_x(stops: &[GradientStop], pos_t: f32, bar_w: i32) -> Option<usize> {
    let tolerance = 6.0 / bar_w.max(1) as f32;
    let mut best: Option<(usize, f32)> = None;
    for (i, stop) in stops.iter().enumerate() {
        let dist = (stop.position - pos_t).abs();
        if dist < tolerance && (best.is_none() || dist < best.unwrap().1) {
            best = Some((i, dist));
        }
    }
    best.map(|(i, _)| i)
}

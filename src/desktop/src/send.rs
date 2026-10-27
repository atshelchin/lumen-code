//! The sender (the web's send.html): pick a file or type text, then the code fills the window.

use crate::app::Root;
use crate::theme::*;
use gpui::{
    canvas, div, fill, point, prelude::*, px, rgba, size, AnyElement, Bounds, Context, Corners, FontWeight, Hsla, Pixels, Rgba, SharedString,
    Window,
};
use lumen_app::app::{Event, Screen, SendPhase, SourceTab};
use lumen_app::engine::send::{CellFrame, StaticLayout};
use lumen_app::geometry::Density;

/// Height of the control bar under the code (its top line included).
pub const BAR_H: f32 = 57.0;

pub fn render(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
    let s = root.view.send.clone();
    let mut stage = div().id("send").size_full().relative();
    if s.phase == SendPhase::Pick {
        stage = stage.child(pick(root, window, cx)).child(top_bar(root));
        return stage;
    }
    // the text box is gone: keys (Space, Enter, Esc) go to the root again
    if root.text_area.read(cx).focus_handle.is_focused(window) {
        root.focus.focus(window, cx);
    }
    let top = title_inset(window);
    // Nothing is ever drawn over the code: it gets the space above the settings and the bar,
    // measured here and reported to the core, which fits the grid (quiet zone and corners) to it.
    let (rt, reported) = (root.rt.clone(), root.reported.clone());
    let code = (root.layout.clone(), root.frame.clone());
    let area = div().id("code").flex_1().min_h_0().w_full().overflow_hidden().child(
        canvas(|_, _, _| {}, move |bounds, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            crate::app::report_area(&rt, &reported, w, h, window.scale_factor());
            if let (Some(layout), Some(frame)) = &code {
                // One layer: its primitives share one z-order, so GPUI does not index
                // tens of thousands of cells in its bounds tree every frame (that
                // indexing was nearly all of the CPU time). Insertion order stacks them.
                window.paint_layer(bounds, |window| paint_code(bounds, layout, frame, window))
            }
        })
        .size_full(),
    );
    let mut view = div().size_full().flex().flex_col().bg(c(INK)).child(div().h(px(top)).flex_none()).child(area);
    if s.settings_open {
        view = view.child(settings(root, window));
    }
    stage = stage.child(view.child(bar(root, window)));
    if s.flash_ask {
        // the first code waits, paused, until the warning is read; nothing else is offered
        stage = stage.child(flash_card(root, top));
    }
    stage
}

/// The photosensitivity warning over the paused first code: play it, or close.
fn flash_card(root: &Root, top: f32) -> AnyElement {
    let s = &root.view.send;
    let (r1, r2) = (root.rt.clone(), root.rt.clone());
    let card = panel_box()
        .id("flash")
        .w(px(440.))
        .max_w_full()
        .p(px(24.))
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .child(icon_tile("warn", AMBER, 56.))
        .child(div().mt(px(14.)).mb(px(8.)).text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child(root.t("flashTitle")))
        .child(div().text_size(px(14.)).line_height(px(21.)).text_color(c(MUTED)).child(s.flash_warn.clone()))
        .child(
            div()
                .mt(px(22.))
                .w_full()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(button("flash-play", root.t("flashPlay"), true, true, move |_, _| r1.dispatch(Event::AcceptFlash)))
                .child(button("flash-close", root.t("close"), false, true, move |_, _| r2.dispatch(Event::CloseSend))),
        );
    // a scrim over the code and the bar: nothing else is offered until the warning is read
    div()
        .id("flash-scrim")
        .block_mouse_except_scroll()
        .absolute()
        .top(px(top))
        .left_0()
        .right_0()
        .bottom_0()
        .bg(rgba(0x04060ac0))
        .flex()
        .items_center()
        .justify_center()
        .p(px(16.))
        .child(card)
        .into_any_element()
}

fn top_bar(root: &Root) -> AnyElement {
    let rt = root.rt.clone();
    div()
        .id("topbar")
        .block_mouse_except_scroll()
        .absolute()
        .top(px(BAR_TOP))
        .left(px(14.))
        .child(
            div()
                .id("home")
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .child(icon("back", 20., c(TEXT)))
                .child(brand_mark(6.))
                .on_click(move |_, _, _| rt.dispatch(Event::Navigate { screen: Screen::Home })),
        )
        .into_any_element()
}

fn pick(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) -> AnyElement {
    let s = root.view.send.clone();
    let rt = root.rt.clone();
    let mut card = div()
        .w(px(520.))
        .max_w_full()
        .px(px(26.))
        .pt(px(34.))
        .pb(px(26.))
        .rounded(px(24.))
        .border_2()
        .border_dashed()
        .border_color(white(0.16))
        .bg(white(0.02))
        .flex()
        .flex_col()
        .items_center()
        .text_center();
    let r1 = rt.clone();
    card = card.child(
        div().mb(px(22.)).child(segmented(
            "tabs",
            vec![(SourceTab::File, root.t("tabFile").into()), (SourceTab::Text, root.t("tabText").into())],
            s.source_tab,
            84.,
            move |tab, _, _| r1.dispatch(Event::SourceTab { tab }),
        )),
    );
    if s.source_tab == SourceTab::File {
        let r2 = rt.clone();
        card = card
            .child(icon_tile("upload", CYAN, 64.))
            .child(div().mt(px(16.)).mb(px(6.)).text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).child(root.t("dropTitle")))
            .child(div().mb(px(22.)).text_color(c(MUTED)).child(s.drop_desc.clone()))
            .child(button("choose", root.t("choose"), true, true, move |_, _| r2.dispatch(Event::PickFile)));
    } else {
        let placeholder: SharedString = root.t("textPh").into();
        let r3 = rt.clone();
        root.text_area.update(cx, |t, _| {
            t.placeholder = placeholder;
            if t.on_submit.is_none() {
                t.on_submit = Some(Box::new(move || r3.dispatch(Event::SendText)));
            }
        });
        let r4 = rt.clone();
        card = card
            .child(root.text_area.clone())
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_between()
                    .mt(px(8.))
                    .mb(px(16.))
                    .px(px(2.))
                    .text_size(px(12.))
                    .text_color(c(FAINT))
                    .child(div().font_family(MONO).child(s.text_meta.clone()))
                    .child(root.t("textKbd")),
            )
            .child(button("send-text", root.t("sendText"), true, s.can_send_text, move |_, _| r4.dispatch(Event::SendText)));
    }
    if let Some(err) = &s.error {
        card = card.child(div().mt(px(14.)).text_size(px(13.)).text_color(c(RED)).child(err.clone()));
    }
    let r5 = rt.clone();
    card = card
        .child(div().mt(px(18.)).text_size(px(12.5)).text_color(c(FAINT)).child(s.drop_hint.clone()))
        // photosensitivity warning, before any code plays
        .child(
            div()
                .mt(px(14.))
                .max_w(px(460.))
                .flex()
                .items_start()
                .gap(px(8.))
                .px(px(12.))
                .py(px(10.))
                .rounded(px(12.))
                .bg(rgba(0xffd60a10))
                .border_1()
                .border_color(rgba(0xffd60a38))
                .child(div().mt(px(1.)).child(icon("warn", 16., c(AMBER))))
                .child(div().flex_1().min_w(px(0.)).text_left().text_size(px(12.5)).line_height(px(18.)).text_color(c(0xe8d48a)).child(s.flash_warn.clone())),
        )
        .child(
            div()
                .id("go-recv")
                .mt(px(18.))
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(13.))
                .text_color(c(MUTED))
                .cursor_pointer()
                .hover(|st| st.text_color(c(TEXT)))
                .child(root.t("goRecv"))
                .child(icon("arrow", 16., c(MUTED)))
                .on_click(move |_, _, _| r5.dispatch(Event::Navigate { screen: Screen::Receive })),
        );
    let _ = window;
    div().id("pick").size_full().overflow_y_scroll().flex().items_center().justify_center().p(px(20.)).pt(px(80.)).child(card).into_any_element()
}

/// The code as rounded quads, every edge on a device pixel (the reference sender's look:
/// rounded finders, round markers, "LED" cells with dark gaps).
fn paint_code(bounds: Bounds<Pixels>, l: &StaticLayout, f: &CellFrame, window: &mut Window) {
    let scale = window.scale_factor();
    let s = l.s as f32 / scale;
    let q = l.quiet;
    let (wq, hq) = (l.x + 2 * q, l.y + 2 * q);
    if f.width as usize != wq || f.height as usize != hq {
        return;
    }
    let (w, h) = (wq as f32 * s, hq as f32 * s);
    let snap = |v: f32| (v * scale).round() / scale;
    let ox = snap(f32::from(bounds.origin.x) + (f32::from(bounds.size.width) - w) / 2.0);
    let oy = snap(f32::from(bounds.origin.y) + (f32::from(bounds.size.height) - h) / 2.0);
    let rgb = |c: [u8; 3]| -> Hsla { Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: 1.0 }.into() };
    let (ink, paper) = (rgb(l.ink), rgb(l.paper));
    let cell = |cx: f32, cy: f32, n: f32, r: f32, color: Hsla, window: &mut Window| {
        let b = Bounds::new(point(px(ox + cx * s), px(oy + cy * s)), size(px(n * s), px(n * s)));
        window.paint_quad(fill(b, color).corner_radii(Corners::all(px(r * s))));
    };
    window.paint_quad(fill(Bounds::new(point(px(ox), px(oy)), size(px(w), px(h))), ink));
    let qf = q as f32;
    for &(fx, fy) in &l.finders {
        let (x, y) = (fx as f32 + qf, fy as f32 + qf);
        cell(x, y, 7.0, 1.5, paper, window);
        cell(x + 1.0, y + 1.0, 5.0, 0.95, ink, window);
        cell(x + 2.0, y + 2.0, 3.0, 0.55, paper, window);
    }
    let (kx, ky) = (l.corner.0 as f32 + qf, l.corner.1 as f32 + qf);
    cell(kx, ky, 7.0, 1.5, paper, window);
    cell(kx + 1.0, ky + 1.0, 5.0, 0.95, ink, window);
    cell(kx + 3.0, ky + 3.0, 1.0, 0.5, paper, window);
    for &(mx, my) in &l.markers {
        cell(mx as f32 + qf, my as f32 + qf, 1.0, 0.32, paper, window);
    }
    // data and header cells: a rounded square of (s - gap) device pixels in each cell
    let inner = (l.s - l.gap) as f32 / scale;
    let radius = px(l.radius as f32 / scale);
    let ink_px = [l.ink[0], l.ink[1], l.ink[2]];
    let mut last = [0u8; 3];
    let mut color = ink;
    for cy in 0..l.y {
        let row = (cy + q) * wq;
        for cx in 0..l.x {
            if !l.data[cy * l.x + cx] {
                continue;
            }
            let i = (row + cx + q) * 4;
            let px3 = [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]];
            if px3 == ink_px {
                continue;
            }
            if px3 != last {
                last = px3;
                color = rgb(px3);
            }
            let b = Bounds::new(point(px(ox + (cx + q) as f32 * s), px(oy + (cy + q) as f32 * s)), size(px(inner), px(inner)));
            window.paint_quad(fill(b, color).corner_radii(Corners::all(radius)));
        }
    }
}

/// A long file name loses its middle, never its extension: ("holiday-ph…", "2024.zip").
fn name_parts(name: &str) -> (String, String) {
    let chars: Vec<char> = name.chars().collect();
    let ext = match chars.iter().rposition(|&c| c == '.') {
        Some(dot) if dot > 0 && chars.len() - dot <= 8 => chars.len() - dot,
        _ => 0,
    };
    let stem = chars.len() - ext;
    let cut = stem - stem.min(4);
    (chars[..cut].iter().collect(), chars[cut..].iter().collect())
}

/// The bar's quiet buttons (`.sendbar .btn.ghost`); `on` marks an open panel.
fn ghost_button(id: &'static str, name: &str, on: bool, f: impl Fn(&mut Window, &mut gpui::App) + 'static) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(44.))
        .flex_none()
        .rounded(px(12.))
        .when(on, |d| d.bg(white(0.12)))
        .hover(|st| st.bg(white(0.08)))
        .cursor_pointer()
        .child(icon(name, 20., c(if on { TEXT } else { MUTED })))
        .on_click(move |_, w, cx| f(w, cx))
        .into_any_element()
}

/// The control bar under the code, always there: play, what is sent, settings, full screen, close.
fn bar(root: &Root, window: &Window) -> AnyElement {
    let s = &root.view.send;
    let (r1, r2, r3) = (root.rt.clone(), root.rt.clone(), root.rt.clone());
    let paused = !s.playing && s.phase == SendPhase::Ready;
    let (head, tail) = name_parts(&s.title);
    let play = div()
        .id("play")
        .flex()
        .items_center()
        .justify_center()
        .size(px(44.))
        .flex_none()
        .rounded_full()
        .bg(c(PAPER))
        .hover(|st| st.bg(gpui::white()))
        .cursor_pointer()
        .child(icon(if s.playing { "pause" } else { "play" }, 20., c(INK)))
        .on_click(move |_, _, _| r1.dispatch(Event::TogglePlay));
    let what = div()
        .flex_1()
        .min_w_0()
        .px(px(6.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .min_w_0()
                .text_size(px(14.))
                .line_height(px(18.))
                .font_weight(FontWeight::SEMIBOLD)
                .whitespace_nowrap()
                .child(div().min_w_0().overflow_hidden().text_ellipsis().child(head))
                .child(div().flex_none().child(tail)),
        )
        .child(
            div()
                .text_size(px(12.5))
                .line_height(px(16.))
                .text_color(c(if paused { AMBER } else { MUTED }))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(s.subtitle.clone()),
        );
    div()
        .id("bar")
        .flex_none()
        .h(px(BAR_H))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(4.))
        .bg(c(PANEL_SOLID))
        .border_t_1()
        .border_color(line())
        .child(play)
        .child(what)
        .child(ghost_button("settings", "settings", s.settings_open, move |_, _| r2.dispatch(Event::ToggleSettings)))
        .child(ghost_button("fullscreen", "fullscreen", window.is_fullscreen(), |w, _| w.toggle_fullscreen()))
        .child(ghost_button("close", "close", false, move |_, _| r3.dispatch(Event::CloseSend)))
        .into_any_element()
}

/// The settings, above the bar; the code shrinks to the space left above them.
fn settings(root: &Root, window: &Window) -> AnyElement {
    let s = &root.view.send;
    let rt = root.rt.clone();
    let (d1, d2, d3, f1) = (rt.clone(), rt.clone(), rt.clone(), rt.clone());
    // the web's grid: as many columns of at least 330 pt as fit, 24 pt apart
    let size = window.viewport_size();
    let avail = f32::from(size.width) - 20.0;
    let cols = ((avail + 24.0) / (330.0 + 24.0)).floor().max(1.0);
    let field_w = ((avail - 24.0 * (cols - 1.0)) / cols).floor();
    let label_w = if field_w < 300.0 { 84.0 } else { 112.0 };
    let field = |label: &str, el: AnyElement| {
        div()
            .w(px(field_w))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().w(px(label_w)).flex_none().text_size(px(13.)).text_color(c(MUTED)).child(label.to_string()))
            .child(div().flex_1().min_w_0().child(el))
    };
    let fps = s.fps;
    let slider_w = (field_w - label_w - 10.0 - 10.0 - 26.0).max(80.0);
    let fields = div()
        .flex()
        .flex_wrap()
        .gap_x(px(24.))
        .gap_y(px(8.))
        .child(field(
            root.t("density"),
            segmented_fill(
                "density",
                vec![
                    (Density::Auto, root.t("dAuto").into()),
                    (Density::Low, root.t("dLow").into()),
                    (Density::High, root.t("dHigh").into()),
                    (Density::Max, root.t("dMax").into()),
                ],
                s.density,
                move |value, _, _| d1.dispatch(Event::SetDensity { value }),
            )
            .into_any_element(),
        ))
        .child(field(root.t("colors"), segmented_fill("mode", vec![(1u8, "4".into()), (2u8, "8".into())], s.mode, move |value, _, _| d2.dispatch(Event::SetMode { value })).into_any_element()))
        .child(field(
            root.t("ecc"),
            segmented_fill("ecc", vec![(0u8, root.t("eLow").into()), (1u8, root.t("eMid").into()), (2u8, root.t("eHigh").into())], s.ecc, move |value, _, _| {
                d3.dispatch(Event::SetEcc { value })
            })
            .into_any_element(),
        ))
        .child(field(
            root.t("speed"),
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(slider(&root.fps_slider, fps as f32, 4.0, 60.0, slider_w, move |v, _, _| f1.dispatch(Event::SetFps { value: v as u32 })))
                .child(div().w(px(26.)).flex_none().text_right().font_family(MONO).text_size(px(13.)).font_weight(FontWeight::BOLD).child(format!("{fps}")))
                .into_any_element(),
        ));
    let mut panel = div()
        .id("settings")
        .flex_none()
        .max_h(px(f32::from(size.height) * 0.5))
        .overflow_y_scroll()
        .px(px(10.))
        .pt(px(12.))
        .pb(px(10.))
        .bg(c(PANEL_SOLID))
        .border_t_1()
        .border_color(line())
        .child(fields);
    if !s.details.is_empty() {
        panel = panel.child(
            div()
                .mt(px(10.))
                .flex()
                .gap(px(6.))
                .text_size(px(11.5))
                .line_height(px(18.))
                .text_color(c(FAINT))
                .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(c(MUTED)).child(root.t("details")))
                .child(div().flex_1().min_w_0().child(s.details.clone())),
        );
    }
    panel.into_any_element()
}

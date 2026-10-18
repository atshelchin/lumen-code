//! The sender (the web's send.html): pick a file or type text, then the code fills the window.

use crate::app::Root;
use crate::theme::*;
use gpui::{
    canvas, div, fill, point, prelude::*, px, size, AnyElement, Bounds, Context, Corners, FontWeight, Hsla, Pixels, Rgba, SharedString,
    Window,
};
use lumen_app::app::{Event, Screen, SendPhase, SourceTab};
use lumen_app::engine::send::{CellFrame, StaticLayout};
use lumen_app::geometry::Density;

pub fn render(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
    let s = root.view.send.clone();
    let mut stage = div().id("send").size_full().relative();
    if s.phase == SendPhase::Pick {
        stage = stage.child(pick(root, window, cx)).child(top_bar(root));
        return stage;
    }
    stage = stage.bg(c(INK));
    if let (Some(layout), Some(frame)) = (root.layout.clone(), root.frame.clone()) {
        let rt = root.rt.clone();
        let top = title_inset(window);
        stage = stage.child(
            div()
                .id("code")
                .absolute()
                .top(px(top))
                .left_0()
                .right_0()
                .bottom_0()
                .cursor_pointer()
                .on_click(move |_, _, _| rt.dispatch(Event::TapCode))
                .child(
                    canvas(|_, _, _| {}, move |bounds, _, window, _| {
                        // One layer: its primitives share one z-order, so GPUI does not index
                        // tens of thousands of cells in its bounds tree every frame (that
                        // indexing was nearly all of the CPU time). Insertion order stacks them.
                        window.paint_layer(bounds, |window| paint_code(bounds, &layout, &frame, window))
                    })
                    .size_full(),
                ),
        );
    }
    if s.dock_visible {
        stage = stage.child(dock(root));
    }
    stage
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

fn dock(root: &Root) -> AnyElement {
    let s = root.view.send.clone();
    let rt = root.rt.clone();
    let (r1, r2, r3, r4, r5) = (rt.clone(), rt.clone(), rt.clone(), rt.clone(), rt.clone());
    let mut stats = div().flex().gap(px(12.)).text_size(px(12.)).text_color(c(MUTED)).overflow_hidden();
    for st in &s.stats {
        stats = stats.child(
            div()
                .flex()
                .gap(px(4.))
                .flex_none()
                .child(st.label.clone())
                .child(div().font_family(MONO).font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(st.value.clone())),
        );
    }
    let mut panel = panel_box()
        .id("dock")
        // clicks on the dock must not reach the code behind it, which toggles the dock (the
        // hide button "did nothing": hidden, then shown again by the same click)
        .block_mouse_except_scroll()
        .w(px(720.))
        .max_w_full()
        .p(px(10.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon_button("play", if s.playing { "pause" } else { "play" }, false, move |_, _| r1.dispatch(Event::TogglePlay)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(14.))
                                .child(status_dot(s.playing && s.phase == SendPhase::Ready, !s.playing))
                                .child(div().font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().whitespace_nowrap().child(s.title.clone()))
                                .child(div().flex_none().font_family(MONO).text_size(px(12.5)).text_color(c(MUTED)).child(s.size.clone())),
                        )
                        .child(stats),
                )
                .child(icon_button("settings", "settings", s.settings_open, move |_, _| r2.dispatch(Event::ToggleSettings)))
                .child(icon_button("fullscreen", "fullscreen", false, |w, _| w.toggle_fullscreen()))
                .child(icon_button("hide", "down", false, move |_, _| r3.dispatch(Event::HideDock)))
                .child(icon_button("close", "close", false, move |_, _| r4.dispatch(Event::CloseSend))),
        );
    if s.settings_open {
        let field = |label: &str, el: AnyElement| div().flex().items_center().gap(px(8.)).child(div().text_size(px(12.)).text_color(c(FAINT)).child(label.to_string())).child(el);
        let (d1, d2, d3) = (rt.clone(), rt.clone(), rt.clone());
        let fps = s.fps;
        let f1 = rt.clone();
        panel = panel.child(
            div()
                .mt(px(10.))
                .pt(px(10.))
                .border_t_1()
                .border_color(line())
                .flex()
                .flex_wrap()
                .gap_x(px(16.))
                .gap_y(px(10.))
                .child(field(
                    root.t("density"),
                    segmented(
                        "density",
                        vec![
                            (Density::Low, root.t("dLow").into()),
                            (Density::Auto, root.t("dAuto").into()),
                            (Density::High, root.t("dHigh").into()),
                            (Density::Max, root.t("dMax").into()),
                        ],
                        s.density,
                        0.,
                        move |value, _, _| d1.dispatch(Event::SetDensity { value }),
                    )
                    .into_any_element(),
                ))
                .child(field(root.t("colors"), segmented("mode", vec![(1u8, "4".into()), (2u8, "8".into())], s.mode, 0., move |value, _, _| d2.dispatch(Event::SetMode { value })).into_any_element()))
                .child(field(
                    root.t("ecc"),
                    segmented("ecc", vec![(0u8, root.t("eLow").into()), (1u8, root.t("eMid").into()), (2u8, root.t("eHigh").into())], s.ecc, 0., move |value, _, _| {
                        d3.dispatch(Event::SetEcc { value })
                    })
                    .into_any_element(),
                ))
                .child(field(
                    root.t("speed"),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(slider(&root.fps_slider, fps as f32, 4.0, 60.0, 180.0, move |v, _, _| f1.dispatch(Event::SetFps { value: v as u32 })))
                        .child(div().w(px(28.)).font_family(MONO).font_weight(FontWeight::BOLD).child(format!("{fps}")))
                        .into_any_element(),
                )),
        );
    }
    let _ = r5;
    div().absolute().bottom(px(12.)).left_0().right_0().flex().justify_center().px(px(8.)).child(panel).into_any_element()
}

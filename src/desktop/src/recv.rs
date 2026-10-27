//! The receiver (the web's recv.html): start screen, webcam with the dashboard, result.

use crate::app::Root;
use crate::home::gradient_text;
use crate::theme::*;
use gpui::{
    canvas, div, img, point, prelude::*, px, AnyElement, Bounds, Context, FontWeight, HighlightStyle, ObjectFit, PathBuilder, Pixels,
    SharedString, StyledText, Window,
};
use lumen_app::app::view::{DoneView, Overlay, Tone};
use lumen_app::app::{Event, RecvPhase, Screen};
use lumen_app::content::{Kind, SpanKind};
use std::sync::Arc;

pub fn render(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) -> impl IntoElement {
    let r = root.view.recv.clone();
    let mut stage = div().id("recv").size_full().relative().bg(gpui::black());
    if r.camera_on || r.camera_starting {
        if let Some(image) = root.preview.clone() {
            stage = stage.child(img(image).absolute().size_full().object_fit(ObjectFit::Cover));
        }
        if let (Some(o), Some(image)) = (r.overlay.clone(), root.preview.clone()) {
            let (iw, ih) = (image.size(0).width.0 as f32, image.size(0).height.0 as f32);
            stage = stage.child(canvas(|_, _, _| {}, move |b, _, window, _| paint_outline(b, &o, iw, ih, window)).absolute().size_full());
        }
    }
    match r.phase {
        RecvPhase::Start => stage = stage.child(start(root)),
        RecvPhase::Scanning => stage = stage.child(hud(root, window, cx)),
        RecvPhase::Done => {
            if let Some(d) = r.done.clone() {
                stage = stage.child(done(root, &d, window, cx));
            }
        }
    }
    // the way home; a reader result has it in its own header instead
    let reader = r.phase == RecvPhase::Done && r.done.as_ref().is_some_and(|d| d.reader);
    if !reader {
        let rt = root.rt.clone();
        let mut bar = div().absolute().top(px(BAR_TOP)).left(px(14.)).flex().items_center().gap(px(10.)).child(
            div()
                .id("home")
                .flex()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .child(icon("back", 20., c(TEXT)))
                .child(brand_mark(6.))
                .on_click(move |_, _, _| rt.dispatch(Event::Navigate { screen: Screen::Home })),
        );
        if r.phase == RecvPhase::Scanning {
            bar = bar.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(6.))
                    .rounded_full()
                    .bg(panel())
                    .border_1()
                    .border_color(line())
                    .text_size(px(13.))
                    .child(status_dot(r.live, false))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(r.status.clone())),
            );
        }
        stage = stage.child(bar);
    }
    stage
}

/// The code outline, from camera-frame pixels onto the cover-fitted preview.
fn paint_outline(b: Bounds<Pixels>, o: &Overlay, iw: f32, ih: f32, window: &mut Window) {
    let (fw, fh) = (o.frame_w as f32, o.frame_h as f32);
    if fw <= 0.0 || iw <= 0.0 {
        return;
    }
    let (bw, bh) = (f32::from(b.size.width), f32::from(b.size.height));
    let k = (bw / iw).max(bh / ih);
    let ox = f32::from(b.origin.x) + (bw - iw * k) / 2.0;
    let oy = f32::from(b.origin.y) + (bh - ih * k) / 2.0;
    let map = |p: [f32; 2]| point(px(ox + p[0] / fw * iw * k), px(oy + p[1] / fh * ih * k));
    let color = c(match o.tone {
        Tone::Good => GREEN,
        Tone::Fair => AMBER,
        Tone::Poor => RED,
    });
    for (width, alpha) in [(9.0, 0.35), (3.0, 1.0)] {
        let mut path = PathBuilder::stroke(px(width));
        path.move_to(map(o.points[0]));
        for p in &o.points[1..] {
            path.line_to(map(*p));
        }
        path.close();
        if let Ok(p) = path.build() {
            window.paint_path(p, color.opacity(alpha));
        }
    }
}

fn start(root: &Root) -> AnyElement {
    let r = &root.view.recv;
    let (r1, r2, r3, r4) = (root.rt.clone(), root.rt.clone(), root.rt.clone(), root.rt.clone());
    let mut inner = div()
        .max_w(px(420.))
        .flex()
        .flex_col()
        .items_center()
        .text_center()
        .child(icon_tile("scan", PINK, 64.))
        .child(div().mt(px(18.)).mb(px(8.)).text_size(px(24.)).font_weight(FontWeight::BOLD).child(root.t("recvStartTitle")))
        .child(div().mb(px(22.)).text_color(c(MUTED)).child(root.t("recvStartDesc")))
        .child(
            div()
                .flex()
                .gap(px(10.))
                .child(button("open-cam", root.t("openCam"), true, !r.camera_starting, move |_, _| r1.dispatch(Event::OpenCamera)))
                .child(button("from-image", root.t("fromImage"), false, true, move |_, _| r2.dispatch(Event::PickImage))),
        );
    if let Some(e) = &r.error {
        inner = inner.child(div().mt(px(14.)).text_size(px(13.)).text_color(c(RED)).child(e.clone()));
        if r.can_open_settings {
            inner = inner.child(div().mt(px(10.)).child(button("settings", root.t("openSettings"), false, true, move |_, _| r3.dispatch(Event::OpenSettings))));
        }
    }
    if let Some(n) = &r.note {
        inner = inner.child(div().mt(px(14.)).text_size(px(13.)).text_color(c(AMBER)).child(n.clone()));
    }
    inner = inner.child(
        div()
            .id("go-send")
            .mt(px(26.))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(13.))
            .text_color(c(MUTED))
            .cursor_pointer()
            .hover(|s| s.text_color(c(TEXT)))
            .child(root.t("goSend"))
            .child(icon("arrow", 16., c(MUTED)))
            .on_click(move |_, _, _| r4.dispatch(Event::Navigate { screen: Screen::Send })),
    );
    let _ = gradient_text;
    div()
        .absolute()
        .size_full()
        .bg(c(BG))
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .child(inner)
        .into_any_element()
}

/// Framing guides: four corners marking where to put the code, until one is found.
fn guides(show: bool) -> AnyElement {
    let corner = || div().absolute().size(px(30.)).border_color(white(0.7));
    let mut g = div().flex_1().min_h_0().relative();
    if show {
        g = g
            .child(corner().top_0().left_0().border_t(px(3.)).border_l(px(3.)).rounded_tl(px(10.)))
            .child(corner().top_0().right_0().border_t(px(3.)).border_r(px(3.)).rounded_tr(px(10.)))
            .child(corner().bottom_0().left_0().border_b(px(3.)).border_l(px(3.)).rounded_bl(px(10.)))
            .child(corner().bottom_0().right_0().border_b(px(3.)).border_r(px(3.)).rounded_br(px(10.)));
    }
    g.into_any_element()
}

/// While scanning: the guides over the camera image, and at the bottom the camera tools (at
/// the image's edge, never over its middle) above the progress panel.
fn hud(root: &Root, window: &Window, cx: &mut Context<Root>) -> AnyElement {
    let r = &root.view.recv;
    let open = root.recv_details;
    let pct = r.progress.clamp(0.0, 1.0);
    let details = div()
        .id("recv-details")
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .h(px(34.))
        .pl(px(12.))
        .pr(px(9.))
        .rounded_full()
        .bg(white(0.04))
        .border_1()
        .border_color(line2())
        .text_size(px(12.5))
        .text_color(c(MUTED))
        .cursor_pointer()
        .hover(|st| st.bg(white(0.08)))
        .child(root.t("details"))
        .child(
            gpui::svg()
                .path("icons/down.svg")
                .size(px(16.))
                .flex_none()
                .text_color(c(MUTED))
                .when(open, |el| el.with_transformation(gpui::Transformation::rotate(gpui::radians(std::f32::consts::PI)))),
        )
        .on_click(cx.listener(|root, _, _, cx| {
            root.toggle_recv_details();
            cx.notify();
        }));
    let mut panel = panel_box()
        .w_full()
        .px(px(16.))
        .py(px(14.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(px(12.))
                .mb(px(10.))
                .child(div().min_w_0().font_weight(FontWeight::SEMIBOLD).overflow_hidden().text_ellipsis().whitespace_nowrap().child(r.name.clone()))
                .child(div().flex_none().font_family(MONO).text_size(px(22.)).font_weight(FontWeight::BOLD).child(r.pct.clone())),
        )
        .child(
            div().h(px(8.)).w_full().rounded_full().bg(white(0.08)).child(
                div()
                    .h_full()
                    .w(gpui::relative(pct))
                    .rounded_full()
                    .bg(gpui::linear_gradient(90., gpui::linear_color_stop(c(CYAN), 0.), gpui::linear_color_stop(c(AMBER), 1.))),
            ),
        )
        // the rate line keeps its height while empty, so nothing moves when data arrives
        .child(
            div()
                .mt(px(6.))
                .min_h(px(36.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.))
                .child(div().min_w_0().text_size(px(13.5)).whitespace_nowrap().overflow_hidden().text_ellipsis().child(r.line.clone()))
                .child(details),
        )
        .child(div().min_h(px(19.5)).text_size(px(13.)).line_height(px(19.5)).text_color(c(if r.tip_ok { MUTED } else { AMBER })).child(r.tip.clone()));
    if open {
        let mut metrics = div().flex().flex_wrap().gap_x(px(18.)).gap_y(px(6.)).text_size(px(12.5)).text_color(c(MUTED));
        for m in &r.metrics {
            metrics = metrics.child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(m.label.clone())
                    .child(div().font_weight(FontWeight::SEMIBOLD).text_color(c(TEXT)).child(m.value.clone())),
            );
        }
        let mut more = div().mt(px(10.)).pt(px(10.)).border_t_1().border_color(line()).flex().flex_col().child(metrics);
        if !r.diag.is_empty() {
            more = more.child(div().mt(px(8.)).font_family(MONO).text_size(px(11.)).line_height(px(16.5)).text_color(c(FAINT)).child(r.diag.clone()));
        }
        panel = panel.child(more);
    }
    let mut dock = div().w(px(560.)).max_w(px(f32::from(window.viewport_size().width) - 20.)).flex().flex_col().gap(px(10.));
    if r.zoom.is_some() || r.can_switch {
        let mut tools = div().flex().justify_end().items_center().gap(px(8.));
        if let Some(z) = &r.zoom {
            let rt = root.rt.clone();
            tools = tools.child(
                div()
                    .id("zoom")
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_h(px(44.))
                    .min_w(px(56.))
                    .px(px(14.))
                    .rounded_full()
                    .bg(gpui::rgba(0x0c1017db))
                    .border_1()
                    .border_color(if z.active { c(CYAN) } else { line2() })
                    .text_size(px(13.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .child(z.label.clone())
                    .on_click(move |_, _, _| rt.dispatch(Event::ResetZoom)),
            );
        }
        if r.can_switch {
            let rt = root.rt.clone();
            tools = tools.child(icon_button("switch", "switch", false, move |_, _| rt.dispatch(Event::SwitchCamera)).bg(gpui::rgba(0x0c1017db)));
        }
        dock = dock.child(tools);
    }
    dock = dock.child(panel);
    let side = (f32::from(window.viewport_size().width) * 0.05).max(22.);
    div()
        .absolute()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        // the guides fill the space between the top bar and the dock
        .child(div().h(px(BAR_TOP + 62.)).flex_none())
        .child(div().flex_1().min_h_0().w_full().px(px(side)).flex().flex_col().child(guides(!r.live)))
        .child(div().h(px(18.)).flex_none())
        .child(dock)
        .child(div().h(px(14.)).flex_none())
        .into_any_element()
}

fn stats_grid(d: &DoneView, cols: usize) -> AnyElement {
    let mut grid = div().w_full().flex().flex_wrap().gap_y(px(10.)).py(px(12.)).px(px(10.)).rounded(px(12.)).bg(white(0.03)).border_1().border_color(line());
    for m in &d.stats {
        grid = grid.child(
            div()
                .w(gpui::relative(1.0 / cols as f32))
                .flex()
                .flex_col()
                .child(div().text_size(px(11.)).text_color(c(FAINT)).child(m.label.clone()))
                .child(div().font_family(MONO).text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(m.value.clone())),
        );
    }
    if let Some(code) = &d.code_line {
        grid = grid.child(div().w_full().text_size(px(11.5)).text_color(c(FAINT)).child(code.clone()));
    }
    grid.into_any_element()
}

fn actions(root: &Root, d: &DoneView) -> AnyElement {
    let text = d.copy_label.is_some();
    let rt = root.rt.clone();
    let mut row = div().flex().flex_wrap().justify_center().gap(px(10.));
    if let Some(copy) = &d.copy_label {
        let rt = rt.clone();
        row = row.child(button("copy", copy.clone(), true, true, move |_, _| rt.dispatch(Event::Copy)));
    }
    if d.link.is_some() {
        let rt = rt.clone();
        row = row.child(button("link", root.t("openLink"), false, true, move |_, _| rt.dispatch(Event::OpenLink)));
    }
    {
        let rt = rt.clone();
        row = row.child(button("save", d.save_label.clone(), d.save_primary && !text, true, move |_, _| rt.dispatch(Event::Save)));
    }
    if d.can_open {
        let rt = rt.clone();
        row = row.child(button("open", root.t("open"), false, true, move |_, _| rt.dispatch(Event::OpenFile)));
    }
    let again = rt.clone();
    row = row.child(button("again", root.t("again"), false, true, move |_, _| again.dispatch(Event::ReceiveAnother)));
    let mut col = div().flex().flex_col().items_center().gap(px(8.)).child(row);
    if let Some(n) = &d.note {
        col = col.child(div().text_size(px(12.5)).text_color(c(if d.note_ok { GREEN } else { RED })).child(n.clone()));
    }
    col.into_any_element()
}

fn badge(small: bool) -> AnyElement {
    let s = if small { 36. } else { 58. };
    div()
        .size(px(s))
        .flex_none()
        .rounded_full()
        .bg(gpui::rgba(0x2bdc7424))
        .flex()
        .items_center()
        .justify_center()
        .child(icon("check", if small { 18. } else { 28. }, c(GREEN)))
        .into_any_element()
}

fn done(root: &mut Root, d: &DoneView, window: &mut Window, cx: &mut Context<Root>) -> AnyElement {
    if !d.reader {
        return div()
            .absolute()
            .size_full()
            .bg(gpui::rgba(0x04060ad9))
            .flex()
            .items_center()
            .justify_center()
            .p(px(14.))
            .child(
                panel_box()
                    .w(px(460.))
                    .max_w_full()
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .text_center()
                    .child(badge(false))
                    .child(div().mt(px(14.)).mb(px(4.)).text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child(d.title.clone()))
                    .child(div().text_size(px(13.5)).text_color(c(MUTED)).child(d.sub.clone()))
                    .child(div().mt(px(16.)).w_full().child(stats_grid(d, 3)))
                    .child(div().mt(px(18.)).child(actions(root, d))),
            )
            .into_any_element();
    }
    let rt = root.rt.clone();
    let (r0, r1, r2, r3, r4, r5) = (rt.clone(), rt.clone(), rt.clone(), rt.clone(), rt.clone(), rt.clone());
    let mut head = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .pt(px(BAR_TOP))
        .pb(px(10.))
        .child(
            // leaving keeps the result: Receive shows it again until "Receive another"
            div()
                .id("home")
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .cursor_pointer()
                .child(icon("back", 20., c(TEXT)))
                .child(brand_mark(6.))
                .on_click(move |_, _, _| r0.dispatch(Event::Navigate { screen: Screen::Home })),
        )
        .child(badge(true))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).whitespace_nowrap().overflow_hidden().text_ellipsis().child(d.title.clone()))
                .child(div().text_size(px(12.5)).text_color(c(MUTED)).whitespace_nowrap().child(d.sub.clone())),
        );
    if d.kind != Some(Kind::Image) {
        head = head
            .child(tool_button("smaller", false, div().text_size(px(12.)).font_weight(FontWeight::BOLD).child("A"), move |_, _| r1.dispatch(Event::SetFont { delta: -1 })))
            .child(tool_button("larger", false, div().text_size(px(18.)).font_weight(FontWeight::BOLD).child("A"), move |_, _| r2.dispatch(Event::SetFont { delta: 1 })));
    }
    head = head.child(
        div()
            .id("details")
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(34.))
            .pl(px(12.))
            .pr(px(9.))
            .rounded_full()
            .bg(white(0.04))
            .border_1()
            .border_color(line2())
            .text_size(px(12.5))
            .text_color(c(MUTED))
            .cursor_pointer()
            .child(root.t("details"))
            .child(icon("down", 16., c(MUTED)))
            .on_click(move |_, _, _| r3.dispatch(Event::ToggleStats)),
    );
    let mut page = div().absolute().size_full().bg(c(BG)).flex().flex_col().child(head);
    if d.stats_open {
        page = page.child(div().px(px(16.)).pb(px(10.)).child(stats_grid(d, 6)));
    }
    if d.json || d.kind == Some(Kind::Text) {
        let mut bar = div().flex().items_center().gap(px(6.)).px(px(16.)).pb(px(8.));
        if d.json {
            bar = bar.child(segmented(
                "view",
                vec![(true, root.t("vPretty").into()), (false, root.t("vRaw").into())],
                d.pretty,
                0.,
                move |pretty, _, _| r4.dispatch(Event::SetJsonView { pretty }),
            ));
        }
        bar = bar.child(div().flex_1()).child(tool_button("wrap", d.wrap, icon("wrap", 18., c(if d.wrap { TEXT } else { MUTED })), move |_, _| {
            r5.dispatch(Event::ToggleWrap)
        }));
        page = page.child(bar);
    }
    let content: AnyElement = if d.kind == Some(Kind::Image) {
        match root.received_image(d.content_rev, window) {
            Some(image) => div().size_full().bg(gpui::black()).child(img(image).size_full().object_fit(ObjectFit::Contain)).into_any_element(),
            None => div().into_any_element(),
        }
    } else {
        reader(root, d, cx)
    };
    page.child(div().flex_1().min_h_0().border_t_1().border_color(line()).bg(gpui::rgba(0x00000038)).child(content))
        .child(div().px(px(16.)).pt(px(10.)).pb(px(12.)).child(actions(root, d)))
        .into_any_element()
}

fn span_color(k: SpanKind) -> u32 {
    match k {
        SpanKind::Key => 0x79d3ff,
        SpanKind::Str => 0x9fe39a,
        SpanKind::Num => 0xffcf5c,
        SpanKind::Bool => 0xff7ab8,
        SpanKind::Null => 0xb9a4ff,
        SpanKind::Punct => 0x6b7488,
    }
}

fn reader(root: &mut Root, d: &DoneView, _cx: &mut Context<Root>) -> AnyElement {
    let Some(content) = root.reader_content(d.content_rev, d.pretty) else { return div().into_any_element() };
    let size = 13.0 + d.font as f32;
    if let Some(lines) = &content.lines {
        // formatted JSON: gutter with line numbers, indentation by depth
        let gut = (lines.len().to_string().len() + 2) as f32 * size * 0.6;
        let mut col = div().flex().flex_col().pt(px(14.)).pb(px(48.)).font_family(MONO).text_size(px(size)).line_height(px(size * 1.6));
        for (i, line) in lines.iter().enumerate().take(20_000) {
            let mut text = String::new();
            let mut hl = Vec::new();
            for sp in &line.spans {
                let a = text.len();
                text.push_str(&sp.text);
                hl.push((a..text.len(), HighlightStyle { color: Some(c(span_color(sp.kind))), ..Default::default() }));
            }
            let body = StyledText::new(text).with_highlights(hl);
            col = col.child(
                div()
                    .flex()
                    .child(div().w(px(gut - size * 0.9)).flex_none().text_right().text_color(c(FAINT).opacity(0.6)).child(SharedString::from(format!("{}", i + 1))))
                    .child(div().w(px(size * 0.9 + line.depth as f32 * 2.0 * size * 0.6)).flex_none())
                    .child(div().flex_1().min_w_0().when(!d.wrap, |el| el.whitespace_nowrap()).child(body)),
            );
        }
        return div().id("reader").size_full().overflow_y_scroll().child(col).into_any_element();
    }
    let text = content.text.clone().unwrap_or_default();
    let plain = d.kind == Some(Kind::Message);
    if plain {
        let size = 17.0 + d.font as f32;
        let hl: Vec<_> = content
            .links
            .iter()
            .map(|&(a, b)| (a..b, HighlightStyle { color: Some(c(CYAN)), underline: Some(gpui::UnderlineStyle { thickness: px(1.), color: Some(c(CYAN).opacity(0.4)), wavy: false }), ..Default::default() }))
            .collect();
        let links: Arc<Vec<(usize, usize)>> = Arc::new(content.links.clone());
        let source = text.clone();
        let styled = StyledText::new(text).with_highlights(hl);
        let ranges: Vec<_> = links.iter().map(|&(a, b)| a..b).collect();
        let interactive = gpui::InteractiveText::new("message", styled).on_click(ranges, move |ix, _, cx| {
            if let Some(&(a, b)) = links.get(ix) {
                cx.open_url(&source[a..b]);
            }
        });
        return div()
            .id("reader")
            .size_full()
            .overflow_y_scroll()
            .child(div().max_w(px(44. * size)).mx_auto().px(px(20.)).pt(px(22.)).pb(px(64.)).text_size(px(size)).line_height(px(size * 1.75)).child(interactive))
            .into_any_element();
    }
    let mut col = div().flex().flex_col().px(px(12.)).pt(px(14.)).pb(px(48.)).font_family(MONO).text_size(px(size)).line_height(px(size * 1.6));
    for line in text.split('\n').take(50_000) {
        col = col.child(div().when(!d.wrap, |el| el.whitespace_nowrap()).child(SharedString::from(line.to_string())));
    }
    div().id("reader").size_full().overflow_y_scroll().child(col).into_any_element()
}

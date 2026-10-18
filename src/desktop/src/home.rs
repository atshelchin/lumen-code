//! The landing screen: brand, hero, the two choices and the facts (the web's index.html).

use crate::app::Root;
use crate::theme::*;
use gpui::{div, prelude::*, px, rgba, AnyElement, Context, FontWeight, HighlightStyle, Hsla, Rgba, StyledText, Window};
use lumen_app::app::{Event, Screen};

/// Text in the hero gradient (cyan → pink → amber), one colour per character.
pub fn gradient_text(text: &str) -> StyledText {
    let stops: [(f32, Rgba); 3] = [(0.0, gpui::rgb(CYAN)), (0.55, gpui::rgb(PINK)), (1.0, gpui::rgb(AMBER))];
    let n = text.chars().count().max(2) as f32 - 1.0;
    let mut highlights = Vec::new();
    for (i, (at, ch)) in text.char_indices().enumerate() {
        let t = i as f32 / n;
        let (a, b) = if t <= stops[1].0 { (stops[0], stops[1]) } else { (stops[1], stops[2]) };
        let k = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
        let mix = Rgba { r: a.1.r + (b.1.r - a.1.r) * k, g: a.1.g + (b.1.g - a.1.g) * k, b: a.1.b + (b.1.b - a.1.b) * k, a: 1.0 };
        let color: Hsla = mix.into();
        highlights.push((at..at + ch.len_utf8(), HighlightStyle { color: Some(color), ..Default::default() }));
    }
    StyledText::new(text.to_string()).with_highlights(highlights)
}

fn choice(root: &Root, id: &'static str, icon_name: &str, tint: u32, title: &str, desc: &str, go: &str, screen: Screen) -> AnyElement {
    let rt = root.rt.clone();
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(22.))
        .rounded(px(20.))
        .bg(gpui::linear_gradient(180., gpui::linear_color_stop(white(0.05), 0.), gpui::linear_color_stop(white(0.02), 1.)))
        .border_1()
        .border_color(line())
        .hover(|s| s.border_color(line2()))
        .cursor_pointer()
        .on_click(move |_, _, _| rt.dispatch(Event::Navigate { screen }))
        .child(icon_tile(icon_name, tint, 46.))
        .child(div().mt(px(6.)).text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
        .child(div().text_size(px(14.)).text_color(c(MUTED)).child(desc.to_string()))
        .child(div().mt_auto().pt(px(8.)).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(go.to_string()))
        .into_any_element()
}

pub fn render(root: &mut Root, window: &mut Window, _cx: &mut Context<Root>) -> impl IntoElement {
    let rt = root.rt.clone();
    let wide = f32::from(window.viewport_size().width) >= 640.0;
    let fact = |t: &str, p: &str| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(root.t(t)))
            .child(div().text_size(px(13.)).text_color(c(MUTED)).child(root.t(p)))
    };
    div().id("home").size_full().overflow_y_scroll().child(
        div()
            .max_w(px(980.))
            .mx_auto()
            .px(px(20.))
            .pt(px(40.))
            .pb(px(24.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex().items_center().gap(px(10.)).child(brand_mark(7.)).child(div().font_weight(FontWeight::SEMIBOLD).child("Lumen Code")))
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("lang")
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(9.))
                            .border_1()
                            .border_color(line())
                            .text_size(px(12.))
                            .text_color(c(MUTED))
                            .cursor_pointer()
                            .hover(|s| s.text_color(c(TEXT)))
                            .child("中 / EN")
                            .on_click(move |_, _, _| rt.dispatch(Event::ToggleLang)),
                    ),
            )
            .child(
                div()
                    .pt(px(80.))
                    .pb(px(44.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .text_size(px(52.))
                            .line_height(px(56.))
                            .font_weight(FontWeight::BOLD)
                            .child(div().child(root.t("heroA")))
                            .child(div().child(gradient_text(root.t("heroB")))),
                    )
                    .child(div().max_w(px(560.)).text_size(px(17.)).text_color(c(MUTED)).child(root.t("heroP"))),
            )
            .child(
                div()
                    .flex()
                    .when(!wide, |d| d.flex_col())
                    .gap(px(14.))
                    .child(choice(root, "choose-send", "screen", CYAN, root.t("sendTitle"), root.t("sendDesc"), root.t("go"), Screen::Send))
                    .child(choice(root, "choose-recv", "scan", PINK, root.t("recvTitle"), root.t("recvDesc"), root.t("go"), Screen::Receive)),
            )
            .child(
                div()
                    .mt(px(56.))
                    .pt(px(26.))
                    .border_t_1()
                    .border_color(line())
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(div().flex().gap(px(24.)).child(fact("f1t", "f1p")).child(fact("f2t", "f2p")))
                    .child(div().flex().gap(px(24.)).child(fact("f3t", "f3p")).child(fact("f4t", "f4p"))),
            )
            .child(div().mt(px(40.)).text_size(px(12.)).text_color(c(FAINT)).child(root.t("foot")))
            .child(div().h(px(1.)).bg(rgba(0x00000000))),
    )
}

//! Design tokens of the web demo (`src/web/css/app.css`) and the few widgets every screen uses.

use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, rgba, size, svg, App, Bounds, ClickEvent, Corners, DispatchPhase, Div, ElementId,
    FontWeight, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Rgba, ScrollWheelEvent, SharedString, Stateful,
    Window,
};
use std::cell::Cell;
use std::rc::Rc;

pub const BG: u32 = 0x07090d;
pub const INK: u32 = 0x0a0d14;
pub const TEXT: u32 = 0xe9edf4;
pub const MUTED: u32 = 0x8b94a7;
pub const FAINT: u32 = 0x5b6476;
pub const CYAN: u32 = 0x22d8ff;
pub const PINK: u32 = 0xff3d9a;
pub const AMBER: u32 = 0xffd60a;
pub const GREEN: u32 = 0x2bdc74;
pub const RED: u32 = 0xff5468;
pub const PAPER: u32 = 0xf2f4f8;
pub const MARK_DARK: u32 = 0x1b2130;

pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// White at an opacity (the web's rgba(255,255,255,a)).
pub fn white(a: f32) -> Hsla {
    Rgba { r: 1.0, g: 1.0, b: 1.0, a }.into()
}

pub fn line() -> Hsla {
    white(0.08)
}
pub fn line2() -> Hsla {
    white(0.14)
}
/// Panels over a playing code or live video: no blur, mostly opaque.
pub fn panel() -> Hsla {
    rgba(0x11151ef2).into()
}

/// Monospace face: Menlo ships with macOS, Consolas with Windows, DejaVu Sans Mono with most
/// Linux desktops (GPUI falls back to a sans face when one is missing).
#[cfg(target_os = "macos")]
pub const MONO: &str = "Menlo";
#[cfg(target_os = "windows")]
pub const MONO: &str = "Consolas";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const MONO: &str = "DejaVu Sans Mono";

/// The strip macOS's transparent title bar lays over the content (its traffic lights); on
/// Windows and Linux the system title bar sits above the content instead.
pub fn title_inset(window: &Window) -> f32 {
    if cfg!(target_os = "macos") && !window.is_fullscreen() {
        28.0
    } else {
        0.0
    }
}

/// Top of the bars that float over the code or the camera (home button, status).
pub const BAR_TOP: f32 = if cfg!(target_os = "macos") { 36.0 } else { 12.0 };

pub fn icon(name: &str, size: f32, color: Hsla) -> impl IntoElement {
    svg().path(SharedString::from(format!("icons/{name}.svg"))).size(px(size)).text_color(color).flex_none()
}

/// The brand mark: a tiny 3×3 code.
pub fn brand_mark(cell: f32) -> Div {
    let colors = [CYAN, MARK_DARK, PINK, MARK_DARK, PAPER, AMBER, MARK_DARK, CYAN, GREEN];
    let gap = cell * 3.0 / 7.0;
    let mut grid = div().flex().flex_col().gap(px(gap));
    for r in 0..3 {
        let mut row = div().flex().gap(px(gap));
        for col in 0..3 {
            row = row.child(div().size(px(cell)).rounded(px(cell * 2.0 / 7.0)).bg(c(colors[r * 3 + col])));
        }
        grid = grid.child(row);
    }
    div()
        .p(px(cell * 6.0 / 7.0))
        .rounded(px(cell * 9.0 / 7.0))
        .bg(c(INK))
        .border_1()
        .border_color(line2())
        .child(grid)
}

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A text button (`.btn`, `.btn.primary`).
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, primary: bool, enabled: bool, on: impl Fn(&mut Window, &mut App) + 'static) -> Stateful<Div> {
    let handler: Handler = Box::new(move |_, w, cx| on(w, cx));
    let mut b = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .min_h(px(44.))
        .px(px(18.))
        .rounded(px(12.))
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into());
    b = if primary {
        b.bg(c(PAPER)).text_color(c(INK)).hover(|s| s.bg(gpui::white()))
    } else {
        b.bg(white(0.04)).border_1().border_color(line2()).text_color(c(TEXT)).hover(|s| s.bg(white(0.08)))
    };
    if enabled {
        b.cursor_pointer().on_click(handler)
    } else {
        b.opacity(0.4)
    }
}

/// A square icon button (`.btn.icon`).
pub fn icon_button(id: impl Into<ElementId>, name: &str, active: bool, on: impl Fn(&mut Window, &mut App) + 'static) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(44.))
        .flex_none()
        .rounded(px(12.))
        .bg(if active { white(0.12) } else { white(0.04) })
        .border_1()
        .border_color(line2())
        .hover(|s| s.bg(white(0.1)))
        .cursor_pointer()
        .child(icon(name, 20., c(TEXT)))
        .on_click(move |_, w, cx| on(w, cx))
}

/// The web's `.tbtn`: small tool buttons in the reader.
pub fn tool_button(id: impl Into<ElementId>, pressed: bool, child: impl IntoElement, on: impl Fn(&mut Window, &mut App) + 'static) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .min_w(px(36.))
        .h(px(34.))
        .px(px(8.))
        .rounded(px(9.))
        .bg(if pressed { white(0.12) } else { white(0.04) })
        .border_1()
        .border_color(if pressed { line2() } else { line() })
        .text_color(if pressed { c(TEXT) } else { c(MUTED) })
        .cursor_pointer()
        .hover(|s| s.bg(white(0.08)))
        .child(child)
        .on_click(move |_, w, cx| on(w, cx))
}

/// A segmented control (`.seg`).
pub fn segmented<T: Clone + PartialEq + 'static>(
    id: &str,
    items: Vec<(T, SharedString)>,
    selected: T,
    min_w: f32,
    pick: impl Fn(T, &mut Window, &mut App) + 'static,
) -> Div {
    let pick = std::rc::Rc::new(pick);
    let mut row = div().flex().p(px(3.)).gap(px(2.)).rounded(px(11.)).bg(white(0.05)).border_1().border_color(line());
    for (i, (value, label)) in items.into_iter().enumerate() {
        let on = value == selected;
        let pick = pick.clone();
        row = row.child(
            div()
                .id(SharedString::from(format!("{id}-{i}")))
                .flex()
                .items_center()
                .justify_center()
                .min_h(px(32.))
                .min_w(px(min_w))
                .px(px(11.))
                .rounded(px(8.))
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .when(on, |d| d.bg(white(0.12)).text_color(c(TEXT)))
                .when(!on, |d| d.text_color(c(MUTED)).hover(|s| s.text_color(c(TEXT))).cursor_pointer())
                .child(label)
                .on_click(move |_, w, cx| {
                    if !on {
                        pick(value.clone(), w, cx)
                    }
                }),
        );
    }
    row
}

pub fn status_dot(live: bool, warn: bool) -> Div {
    let dot = div().size(px(8.)).rounded_full().bg(c(if live { GREEN } else if warn { AMBER } else { FAINT }));
    div().size(px(16.)).flex().items_center().justify_center().when(live, |d| d.rounded_full().bg(rgba(0x2bdc7426))).child(dot)
}

/// Rounded tinted square holding an icon (`.drop-art`, `.choice .ico`).
pub fn icon_tile(name: &str, tint: u32, size: f32) -> Div {
    let mut bg: Rgba = rgb(tint);
    bg.a = 0.11;
    div().size(px(size)).flex_none().rounded(px(size * 0.28)).bg(bg).flex().items_center().justify_center().child(icon(name, size * 0.46, c(tint)))
}

pub fn panel_box() -> Div {
    div().rounded(px(16.)).bg(panel()).border_1().border_color(line()).shadow_lg()
}

/// Drag state of a slider that outlives each render.
#[derive(Clone, Default)]
pub struct SliderState {
    pub dragging: Rc<Cell<bool>>,
    scroll: Rc<Cell<f32>>,
    last: Rc<Cell<f32>>,
}

/// The web's range input: a track, a cyan fill and a round thumb. Drag it, click anywhere on
/// it, or scroll over it; `on_change` gets each new whole-step value once.
pub fn slider(
    state: &SliderState,
    value: f32,
    min: f32,
    max: f32,
    width: f32,
    on_change: impl Fn(f32, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let st = state.clone();
    let on_change = Rc::new(on_change);
    let body = canvas(
        |_, _, _| {},
        move |bounds: Bounds<gpui::Pixels>, _, window, _| {
            let r = 9.0f32;
            let left = f32::from(bounds.origin.x) + r;
            let span = (f32::from(bounds.size.width) - 2.0 * r).max(1.0);
            let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.0;
            let f = ((value - min) / (max - min)).clamp(0.0, 1.0);
            let x = left + f * span;
            let track = |x0: f32, x1: f32, color: Hsla, window: &mut Window| {
                let b = Bounds::new(point(px(x0), px(cy - 2.0)), size(px((x1 - x0).max(0.0)), px(4.0)));
                window.paint_quad(fill(b, color).corner_radii(Corners::all(px(2.0))));
            };
            track(left, left + span, line2(), window);
            track(left, x, c(CYAN), window);
            let thumb = |rad: f32, color: Hsla, dy: f32, window: &mut Window| {
                let b = Bounds::new(point(px(x - rad), px(cy - rad + dy)), size(px(2.0 * rad), px(2.0 * rad)));
                window.paint_quad(fill(b, color).corner_radii(Corners::all(px(rad))));
            };
            thumb(r + 1.0, Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.3 }.into(), 1.0, window);
            thumb(r, gpui::white(), 0.0, window);

            let pick = move |x: f32| (min + ((x - left) / span).clamp(0.0, 1.0) * (max - min)).round();
            let emit = {
                let (st, on_change) = (st.clone(), on_change.clone());
                move |v: f32, window: &mut Window, cx: &mut App| {
                    if v != st.last.get() || v != value {
                        st.last.set(v);
                        on_change(v, window, cx);
                    }
                }
            };
            {
                let (st, emit) = (st.clone(), emit.clone());
                window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && ev.button == MouseButton::Left && bounds.contains(&ev.position) {
                        st.dragging.set(true);
                        emit(pick(f32::from(ev.position.x)), window, cx);
                    }
                });
            }
            {
                let (st, emit) = (st.clone(), emit.clone());
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || !st.dragging.get() {
                        return;
                    }
                    if ev.pressed_button == Some(MouseButton::Left) {
                        emit(pick(f32::from(ev.position.x)), window, cx);
                    } else {
                        st.dragging.set(false);
                    }
                });
            }
            {
                let st = st.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, _, _, _| st.dragging.set(false));
            }
            {
                let (st, emit) = (st.clone(), emit.clone());
                window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || !bounds.contains(&ev.position) {
                        return;
                    }
                    let d = ev.delta.pixel_delta(px(16.0));
                    let acc = st.scroll.get() + f32::from(d.x) - f32::from(d.y);
                    let steps = (acc / 12.0).trunc();
                    st.scroll.set(acc - steps * 12.0);
                    if steps != 0.0 {
                        emit((value + steps).clamp(min, max), window, cx);
                    }
                });
            }
        },
    )
    .size_full();
    div().w(px(width)).h(px(32.)).cursor_pointer().child(body)
}

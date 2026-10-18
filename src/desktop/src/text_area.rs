//! A multi-line text box for the sender's Text tab: typing (with input methods, so Chinese
//! and Japanese work), paste, select-all, backspace, ⌘↵ to send. The caret stays at the end —
//! enough for writing or pasting a message without a full editor.

use gpui::{
    actions, canvas, div, point, prelude::*, px, App, Bounds, ClipboardItem, Context, CursorStyle, ElementInputHandler, EntityInputHandler,
    FocusHandle, KeyBinding, MouseButton, Pixels, Point, SharedString, UTF16Selection, Window,
};
use std::ops::Range;

use crate::theme::{c, white, CYAN, FAINT, TEXT};

actions!(lumen_text, [Backspace, Newline, Paste, Copy, SelectAll, Submit]);

pub fn bind_keys(cx: &mut App) {
    let ctx = Some("LumenText");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("cmd-enter", Submit, ctx),
        KeyBinding::new("ctrl-enter", Submit, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
    ]);
}

pub struct TextArea {
    pub focus_handle: FocusHandle,
    pub content: String,
    /// The input method's provisional text (at the end).
    marked: Option<Range<usize>>,
    /// ⌘A: the next edit replaces everything.
    all: bool,
    on_change: Box<dyn Fn(String)>,
    pub on_submit: Option<Box<dyn Fn()>>,
    pub placeholder: SharedString,
    bounds: Option<Bounds<Pixels>>,
}

impl TextArea {
    pub fn new(cx: &mut Context<Self>, on_change: impl Fn(String) + 'static) -> Self {
        TextArea {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            marked: None,
            all: false,
            on_change: Box::new(on_change),
            on_submit: None,
            placeholder: SharedString::default(),
            bounds: None,
        }
    }

    fn edited(&mut self, cx: &mut Context<Self>) {
        (self.on_change)(self.content.clone());
        cx.notify();
    }

    fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.all {
            self.content.clear();
            self.all = false;
        }
        if let Some(r) = self.marked.take() {
            self.content.replace_range(r, "");
        }
        self.content.push_str(text);
        self.edited(cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.all {
            self.content.clear();
            self.all = false;
        } else {
            self.content.pop();
        }
        self.edited(cx);
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.insert("\n", cx);
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert(&text, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.content.clone()));
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.all = !self.content.is_empty();
        cx.notify();
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, _: &mut Context<Self>) {
        if let Some(f) = &self.on_submit {
            f();
        }
    }

    fn to_utf16(&self, offset: usize) -> usize {
        self.content[..offset.min(self.content.len())].encode_utf16().count()
    }

    fn from_utf16(&self, offset: usize) -> usize {
        let mut u = 0;
        for (i, ch) in self.content.char_indices() {
            if u >= offset {
                return i;
            }
            u += ch.len_utf16();
        }
        self.content.len()
    }
}

impl EntityInputHandler for TextArea {
    fn text_for_range(&mut self, range: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let r = self.from_utf16(range.start)..self.from_utf16(range.end);
        actual.replace(self.to_utf16(r.start)..self.to_utf16(r.end));
        Some(self.content[r].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = self.to_utf16(self.content.len());
        Some(UTF16Selection { range: if self.all { 0..end } else { end..end }, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.to_utf16(r.start)..self.to_utf16(r.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        // the caret is at the end: a range ending there replaces its tail, anything else appends
        if let (Some(r), None) = (range, &self.marked) {
            let start = self.from_utf16(r.start);
            if self.from_utf16(r.end) == self.content.len() && start < self.content.len() {
                self.content.truncate(start);
            }
        }
        self.insert(text, cx);
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        if self.all {
            self.content.clear();
            self.all = false;
        }
        if let Some(r) = self.marked.take() {
            self.content.replace_range(r, "");
        }
        let start = self.content.len();
        self.content.push_str(text);
        self.marked = if text.is_empty() { None } else { Some(start..self.content.len()) };
        cx.notify();
    }

    fn bounds_for_range(&mut self, _: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        // the input method's candidate window: below the box's bottom-left
        let b = self.bounds.unwrap_or(bounds);
        Some(Bounds::from_corners(point(b.left() + px(14.), b.bottom() - px(28.)), point(b.left() + px(15.), b.bottom() - px(8.))))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.to_utf16(self.content.len()))
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let entity = cx.entity();
        let focus = self.focus_handle.clone();
        let body = if self.content.is_empty() {
            div().text_color(c(FAINT)).child(self.placeholder.clone())
        } else {
            let mut d = div().text_color(c(TEXT)).whitespace_normal();
            if self.all {
                d = d.bg(gpui::rgba(0x22d8ff40));
            }
            d.child(self.content.clone())
        };
        div()
            .id("text-area")
            .key_context("LumenText")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.focus_handle.focus(window, cx);
                this.all = false;
                cx.notify();
            }))
            .cursor(CursorStyle::IBeam)
            .relative()
            .w_full()
            .min_h(px(190.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .p(px(14.))
            .rounded(px(14.))
            .bg(gpui::rgba(0x0000004d))
            .border_1()
            .border_color(if focused { gpui::rgba(0x22d8ff8c).into() } else { white(0.14) })
            .text_size(px(15.))
            .line_height(px(22.))
            .child(
                div().flex().flex_wrap().items_end().child(body).when(focused && !self.all, |d| {
                    d.child(div().w(px(2.)).h(px(18.)).mb(px(2.)).bg(c(CYAN)))
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        entity.update(cx, |t, _| t.bounds = Some(bounds));
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity.clone()), cx);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
    }
}

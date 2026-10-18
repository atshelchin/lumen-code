//! Lumen Code for the desktop: the GPUI shell over the shared Rust core.
//!
//! The core (`src/app/core`) decides everything; this crate lays out its view model, forwards
//! input, and performs the operations it asks for (files, clipboard, webcam, screen). It drives
//! the core's runtime directly — no FFI, no JSON.

// a GUI app on Windows: no console window beside it
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod camera;
mod home;
mod platform;
mod recv;
mod script;
mod send;
mod text_area;
mod theme;

use gpui::{
    actions, point, px, size, App, AppContext, AssetSource, Bounds, KeyBinding, Menu, MenuItem, QuitMode, SharedString, TitlebarOptions,
    WindowBounds, WindowOptions,
};
use std::borrow::Cow;

actions!(lumen, [Quit, ToggleFullScreen]);

/// The SVG icons, compiled in.
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),*) => {
                match path {
                    $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice()),)*
                    _ => None,
                }
            };
        }
        let bytes = icons!(
            "arrow", "back", "check", "close", "down", "fullscreen", "pause", "play", "scan", "screen", "settings", "switch", "upload", "wrap"
        );
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

fn main() {
    let application = gpui_platform::application().with_assets(Assets).with_quit_mode(QuitMode::LastWindowClosed);
    application.run(|cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        if cfg!(target_os = "macos") {
            cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None)]);
        } else {
            // Windows closes with Alt-F4 by itself
            cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None), KeyBinding::new("f11", ToggleFullScreen, None)]);
        }
        text_area::bind_keys(cx);
        cx.set_menus(vec![Menu { name: "Lumen".into(), items: vec![MenuItem::action("Quit Lumen", Quit)], disabled: false }]);

        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // macOS: content under a transparent title bar with the traffic lights; Windows and
                // Linux: the system's title bar, with its buttons (GPUI would leave them to the app)
                titlebar: Some(TitlebarOptions {
                    title: Some("Lumen Code".into()),
                    appears_transparent: cfg!(target_os = "macos"),
                    traffic_light_position: cfg!(target_os = "macos").then(|| point(px(14.), px(14.))),
                }),
                window_min_size: Some(size(px(560.), px(480.))),
                app_id: Some("com.shelchin.lumencode".into()),
                focus: true,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Root::new(window, cx)),
        )
        .expect("window");
        cx.activate(true);
    });
}

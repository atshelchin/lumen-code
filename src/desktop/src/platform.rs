//! Desktop services the core asks for: preferences, the screen kept on, file locations.

use gpui::RenderImage;
use std::path::PathBuf;
#[cfg_attr(target_os = "windows", allow(unused_imports))] // Windows keeps the display on without a helper process
use std::process::{Child, Command};

/// The system language as a BCP 47 tag.
pub fn locale() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(out) = Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output() {
        let s = String::from_utf8_lossy(&out.stdout);
        if let Some(first) = s.split('"').nth(1) {
            return first.to_string();
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(tag) = windows::user_locale() {
        return tag;
    }
    // Linux: LC_ALL, then LC_MESSAGES, then LANG ("zh_CN.UTF-8")
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX")
        .unwrap_or_else(|| "en".into())
}

fn config_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    return home.map(|h| h.join("Library/Application Support/Lumen"));
    #[cfg(target_os = "windows")]
    return std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Lumen"));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or(home.map(|h| h.join(".config"))).map(|d| d.join("lumen"));
}

pub fn load_prefs() -> Option<String> {
    std::fs::read_to_string(config_dir()?.join("prefs.json")).ok()
}

pub fn save_prefs(json: &str) {
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("prefs.json"), json);
    }
}

/// The shell's own preferences, beside the core's `prefs.json` (which the core rewrites from
/// its own fields): whether the receive panel's Details are open.
const SHELL_PREFS: &str = "desktop.json";

pub fn load_recv_details() -> bool {
    config_dir()
        .and_then(|d| std::fs::read_to_string(d.join(SHELL_PREFS)).ok())
        .is_some_and(|s| s.replace(char::is_whitespace, "").contains("\"recvDetails\":true"))
}

pub fn save_recv_details(open: bool) {
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(SHELL_PREFS), format!("{{\"recvDetails\":{open}}}\n"));
    }
}

pub fn downloads_dir() -> PathBuf {
    // Windows sets USERPROFILE, not HOME
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    home.map(|h| h.join("Downloads")).filter(|d| d.is_dir()).unwrap_or_else(std::env::temp_dir)
}

/// A fresh temporary path with the file's own name (for "Open").
pub fn temp_file(name: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join("lumen-received");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let safe: String = name.chars().map(|c| if c == '/' || c == '\\' { '_' } else { c }).collect();
    Some(dir.join(if safe.trim().is_empty() { "file".into() } else { safe }))
}

/// Keeps the display awake while a code is shown or the webcam scans.
#[derive(Default)]
pub struct KeepAwake(Option<Child>, bool);

impl KeepAwake {
    pub fn set(&mut self, on: bool) {
        #[cfg(target_os = "windows")]
        if on != self.1 {
            // a thread state: set and cleared on the UI thread, which lives as long as the app
            windows::keep_display_on(on);
        }
        self.1 = on;
        if !on {
            if let Some(mut c) = self.0.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            return;
        }
        if self.0.is_some() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            // `caffeinate -d` holds a display-sleep assertion for as long as it runs; `-w`
            // ends it with this process, even after a crash.
            let pid = std::process::id().to_string();
            self.0 = Command::new("caffeinate").args(["-d", "-i", "-w", &pid]).spawn().ok();
        }
        #[cfg(target_os = "linux")]
        {
            self.0 = Command::new("systemd-inhibit")
                .args(["--what=idle", "--who=Lumen", "--why=Showing a transfer code", "sleep", "infinity"])
                .spawn()
                .ok();
        }
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        self.set(false);
    }
}

/// An RGBA image as a GPUI image (premultiplied BGRA; our images are opaque).
pub fn render_image(mut rgba: image::RgbaImage) -> RenderImage {
    for p in rgba.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    RenderImage::new(vec![image::Frame::new(rgba)])
}

#[cfg(target_os = "windows")]
mod windows {
    // two kernel32 calls; not worth a bindings crate
    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
        fn GetUserDefaultLocaleName(name: *mut u16, len: i32) -> i32;
    }
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;

    pub fn keep_display_on(on: bool) {
        let flags = if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED } else { ES_CONTINUOUS };
        // SAFETY: a plain Win32 call without pointers
        unsafe { SetThreadExecutionState(flags) };
    }

    /// The user's locale, as "zh-CN".
    pub fn user_locale() -> Option<String> {
        let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
        // SAFETY: the buffer and its length in UTF-16 units
        let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
    }
}

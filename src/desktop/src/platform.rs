//! Desktop services the core asks for: preferences, the screen kept on, file locations.

use gpui::RenderImage;
use std::path::PathBuf;
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
    std::env::var("LANG").unwrap_or_else(|_| "en".into())
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

pub fn downloads_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).map(|h| h.join("Downloads")).filter(|d| d.is_dir()).unwrap_or_else(std::env::temp_dir)
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
pub struct KeepAwake(Option<Child>);

impl KeepAwake {
    pub fn set(&mut self, on: bool) {
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

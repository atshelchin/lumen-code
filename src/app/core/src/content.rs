//! What a received file is, and how the reader shows it: text messages, JSON (formatted with
//! syntax classes, literals kept verbatim), clickable links, images. A port of the web demo's
//! `text-view.js` and of the classification in `recv-app.js`, so every shell shows the same.

use serde::{Deserialize, Serialize};

/// Text messages travel as a UTF-8 file with this name; receivers show them as text.
pub const TEXT_NAME: &str = "message.txt";
/// Larger files are never decoded as text.
pub const TEXT_LIMIT: usize = 4_000_000;
/// Display limits only: Copy and Save always use the complete text.
pub const TEXT_DISPLAY_LIMIT: usize = 1_000_000;
/// Larger JSON is formatted without colour classes.
pub const HIGHLIGHT_LIMIT: usize = 300_000;

const TEXTY: &[&str] = &[
    "txt", "md", "json", "csv", "log", "js", "ts", "py", "xml", "yaml", "yml", "ini", "conf", "sh", "c", "h", "cpp", "go", "rs",
    "java", "css",
];

const MIME: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("heic", "image/heic"),
    ("heif", "image/heif"),
    ("bmp", "image/bmp"),
    ("avif", "image/avif"),
    ("svg", "image/svg+xml"),
    ("pdf", "application/pdf"),
    ("txt", "text/plain"),
    ("md", "text/markdown"),
    ("json", "application/json"),
    ("csv", "text/csv"),
    ("html", "text/html"),
    ("mp4", "video/mp4"),
    ("mov", "video/quicktime"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/wav"),
    ("zip", "application/zip"),
];

pub fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.to_ascii_lowercase(),
        None => String::new(),
    }
}

pub fn mime(name: &str) -> &'static str {
    let ext = extension(name);
    MIME.iter().find(|(e, _)| *e == ext).map_or("application/octet-stream", |(_, m)| m)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A message typed on the sender (`message.txt`)
    Message,
    /// A JSON document (message or file)
    Json,
    /// A text file
    Text,
    /// A raster image the platform can show
    Image,
    /// Anything else
    File,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classified {
    pub kind: Kind,
    /// Whether the file came from the sender's text tab
    pub message: bool,
    /// Characters, for text
    pub chars: Option<u64>,
    /// Formatted lines, for JSON
    pub lines: Option<u64>,
    /// The whole text is one http(s) link
    pub link: Option<String>,
    pub mime: String,
}

/// Decode as text when the name says it is text (strict UTF-8).
pub fn as_text(name: &str, bytes: &[u8]) -> Option<String> {
    if (name != TEXT_NAME && !TEXTY.contains(&extension(name).as_str())) || bytes.len() > TEXT_LIMIT {
        return None;
    }
    String::from_utf8(bytes.to_vec()).ok()
}

pub fn classify(name: &str, bytes: &[u8]) -> (Classified, Option<String>) {
    let text = as_text(name, bytes);
    let message = text.is_some() && name == TEXT_NAME;
    let mime = mime(name).to_string();
    let mut c = Classified { kind: Kind::File, message, chars: None, lines: None, link: None, mime };
    if let Some(t) = &text {
        c.chars = Some(t.chars().count() as u64);
        let trimmed = t.trim();
        if is_link(trimmed) {
            c.link = Some(trimmed.to_string());
        }
        if is_json(t) {
            c.kind = Kind::Json;
            c.lines = Some(format_json(t, false).len() as u64);
        } else {
            c.kind = if message { Kind::Message } else { Kind::Text };
        }
    } else if c.mime.starts_with("image/") && c.mime != "image/svg+xml" {
        c.kind = Kind::Image;
    }
    (c, text)
}

fn is_link(s: &str) -> bool {
    let lower = s.get(..8).map(|p| p.to_ascii_lowercase()).unwrap_or_default();
    (lower.starts_with("http://") || lower.starts_with("https://")) && !s.chars().any(char::is_whitespace) && s.len() > 8
}

/// JSON is shown formatted only when the whole text is an object or an array.
pub fn is_json(text: &str) -> bool {
    let s = text.trim();
    let (Some(a), Some(z)) = (s.chars().next(), s.chars().last()) else { return false };
    if !((a == '{' && z == '}') || (a == '[' && z == ']')) {
        return false;
    }
    serde_json::from_str::<serde::de::IgnoredAny>(s).is_ok()
}

/// Syntax class of a span: key, string, number, boolean, null, punctuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Key,
    Str,
    Num,
    Bool,
    Null,
    Punct,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub kind: SpanKind,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonLine {
    /// Nesting depth (two spaces each); the reader indents wrapped lines under their start.
    pub depth: u32,
    pub spans: Vec<Span>,
}

impl JsonLine {
    pub fn plain(&self) -> String {
        let mut s = " ".repeat(self.depth as usize * 2);
        for sp in &self.spans {
            s.push_str(&sp.text);
        }
        s
    }
}

/// JSON tokens exactly as written: strings, numbers, literals and punctuation.
fn tokens(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    loop {
        while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let start = i;
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                if i >= b.len() {
                    break;
                }
                i += 1;
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => i += 1,
            b'-' | b'0'..=b'9' => {
                i += 1;
                while i < b.len() && matches!(b[i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') {
                    i += 1;
                }
            }
            b't' if text[i..].starts_with("true") => i += 4,
            b'f' if text[i..].starts_with("false") => i += 5,
            b'n' if text[i..].starts_with("null") => i += 4,
            _ => break,
        }
        out.push(&text[start..i]);
    }
    out
}

/// Re-indents valid JSON token by token, so number and string literals stay exactly as
/// received (parsing and printing would round large integers and drop duplicate keys).
/// With `classes == false` every span is `Punct` except the text itself (cheaper for huge files).
pub fn format_json(text: &str, classes: bool) -> Vec<JsonLine> {
    let toks = tokens(text);
    let mut lines = Vec::new();
    let mut depth: u32 = 0;
    let mut cur = JsonLine::default();
    let put = |cur: &mut JsonLine, kind: SpanKind, s: &str| {
        let kind = if classes { kind } else { SpanKind::Str };
        if let Some(last) = cur.spans.last_mut() {
            if last.kind == kind && !classes {
                last.text.push_str(s);
                return;
            }
        }
        cur.spans.push(Span { kind, text: s.to_string() });
    };
    let mut i = 0;
    while i < toks.len() {
        let tk = toks[i];
        let c = tk.as_bytes()[0];
        match c {
            b'{' | b'[' => {
                let close = if c == b'{' { "}" } else { "]" };
                if toks.get(i + 1) == Some(&close) {
                    put(&mut cur, SpanKind::Punct, &format!("{tk}{close}"));
                    i += 2;
                    continue;
                }
                put(&mut cur, SpanKind::Punct, tk);
                lines.push(std::mem::take(&mut cur));
                depth += 1;
                cur.depth = depth;
            }
            b'}' | b']' => {
                lines.push(std::mem::take(&mut cur));
                depth = depth.saturating_sub(1);
                cur.depth = depth;
                put(&mut cur, SpanKind::Punct, tk);
            }
            b',' => {
                put(&mut cur, SpanKind::Punct, tk);
                lines.push(std::mem::take(&mut cur));
                cur.depth = depth;
            }
            b':' => put(&mut cur, SpanKind::Punct, ": "),
            b'"' => {
                let kind = if toks.get(i + 1) == Some(&":") { SpanKind::Key } else { SpanKind::Str };
                put(&mut cur, kind, tk);
            }
            b't' | b'f' => put(&mut cur, SpanKind::Bool, tk),
            b'n' => put(&mut cur, SpanKind::Null, tk),
            _ => put(&mut cur, SpanKind::Num, tk),
        }
        i += 1;
    }
    lines.push(cur);
    lines
}

/// http(s) links: printable ASCII only (Chinese text often follows a link without a space);
/// trailing punctuation stays outside the link. Returns byte ranges.
pub fn links(text: &str) -> Vec<(usize, usize)> {
    fn body(c: u8) -> bool {
        matches!(c, b'!' | b'#'..=b'&' | b'('..=b';' | b'=' | b'?'..=b'_' | b'a'..=b'~')
    }
    fn end(c: u8) -> bool {
        matches!(c, b'#'..=b'&' | b'*'..=b'+' | b'/'..=b'9' | b'=' | b'@'..=b'Z' | b'\\' | b'^'..=b'_' | b'a'..=b'z' | b'|' | b'~')
    }
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 8 <= b.len() {
        let rest = &b[i..];
        let scheme = if rest.len() >= 8 && rest[..8].eq_ignore_ascii_case(b"https://") {
            8
        } else if rest[..7].eq_ignore_ascii_case(b"http://") {
            7
        } else {
            0
        };
        let boundary = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if scheme == 0 || !boundary {
            i += 1;
            continue;
        }
        let mut j = i + scheme;
        while j < b.len() && body(b[j]) {
            j += 1;
        }
        while j > i + scheme && !end(b[j - 1]) {
            j -= 1;
        }
        if j > i + scheme {
            out.push((i, j));
            i = j;
        } else {
            i += scheme;
        }
    }
    out
}

/// [`links`] as UTF-16 offsets (Swift `NSRange`, Kotlin `String` indices).
pub fn links_utf16(text: &str) -> Vec<(usize, usize)> {
    let ranges = links(text);
    if ranges.is_empty() {
        return ranges;
    }
    let mut out = Vec::with_capacity(ranges.len());
    let mut u16_at = 0usize;
    let mut byte_at = 0usize;
    let mut chars = text.char_indices().peekable();
    let mut to_u16 = |target: usize, u16_at: &mut usize, byte_at: &mut usize| {
        while *byte_at < target {
            match chars.next() {
                Some((_, c)) => {
                    *u16_at += c.len_utf16();
                    *byte_at += c.len_utf8();
                }
                None => break,
            }
        }
        *u16_at
    };
    for (s, e) in ranges {
        let a = to_u16(s, &mut u16_at, &mut byte_at);
        let z = to_u16(e, &mut u16_at, &mut byte_at);
        out.push((a, z));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_keeps_literals() {
        let t = r#"{"a":12345678901234567890,"b":[],"c":{"d":true,"e":null},"f":[1.50,"x"]}"#;
        assert!(is_json(t));
        let lines: Vec<String> = format_json(t, true).iter().map(JsonLine::plain).collect();
        assert_eq!(
            lines,
            vec![
                "{",
                "  \"a\": 12345678901234567890,",
                "  \"b\": [],",
                "  \"c\": {",
                "    \"d\": true,",
                "    \"e\": null",
                "  },",
                "  \"f\": [",
                "    1.50,",
                "    \"x\"",
                "  ]",
                "}"
            ]
        );
        let plain: Vec<String> = format_json(t, false).iter().map(JsonLine::plain).collect();
        assert_eq!(plain, lines);
        assert!(!is_json("{oops}"));
        assert!(!is_json("42"));
    }

    #[test]
    fn link_ranges() {
        let t = "见 https://example.com/a?b=1。and (http://x.org/y). foohttp://no";
        let got: Vec<&str> = links(t).iter().map(|&(a, b)| &t[a..b]).collect();
        assert_eq!(got, vec!["https://example.com/a?b=1", "http://x.org/y"]);
        let u = links_utf16(t);
        assert_eq!(u[0], (2, 2 + 25));
    }

    #[test]
    fn classification() {
        let (c, t) = classify(TEXT_NAME, "https://lumen.code/x".as_bytes());
        assert_eq!(c.kind, Kind::Message);
        assert_eq!(c.link.as_deref(), Some("https://lumen.code/x"));
        assert!(t.is_some());
        let (c, _) = classify("a.json", b"[1,2]");
        assert_eq!((c.kind, c.lines), (Kind::Json, Some(4)));
        let (c, t) = classify("photo.JPG", &[0xff, 0xd8, 0xff]);
        assert_eq!((c.kind, t), (Kind::Image, None));
        let (c, _) = classify("blob.bin", &[1, 2, 3]);
        assert_eq!(c.kind, Kind::File);
    }
}

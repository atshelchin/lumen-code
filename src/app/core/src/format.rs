//! Number formatting shared by every screen (the web demo's fmtBytes / fmtTime).

/// `1023 B`, `4.2 KB`, `87 KB`, `1.3 MB`.
pub fn bytes(n: f64) -> String {
    if n < 1024.0 {
        format!("{} B", n.round() as u64)
    } else if n < 1_048_576.0 {
        let kb = n / 1024.0;
        if n < 10_240.0 {
            format!("{kb:.1} KB")
        } else {
            format!("{kb:.0} KB")
        }
    } else {
        format!("{:.1} MB", n / 1_048_576.0)
    }
}

/// `42s`, `3m07s`; `–` when unknown.
pub fn time(s: f64) -> String {
    if !s.is_finite() || s < 0.0 {
        return "–".into();
    }
    if s < 60.0 {
        return format!("{}s", s.ceil() as u64);
    }
    format!("{}m{:02}s", (s / 60.0).floor() as u64, (s % 60.0).ceil() as u64)
}

/// Thousands separators, as `toLocaleString` would print them.
pub fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A received file name comes from the sender (PROTOCOL §13): never a path, and without control
/// or bidirectional-override characters (which can disguise an extension). Empty, `.` and `..`
/// become "file".
pub fn safe_name(name: &str) -> String {
    let hidden = |c: char| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
    let clean: String = name
        .chars()
        .filter(|&c| !hidden(c))
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    let clean = clean.trim();
    if clean.is_empty() || clean == "." || clean == ".." {
        "file".into()
    } else {
        clean.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_names_are_not_paths() {
        assert_eq!(safe_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(safe_name("a\\b:c"), "a_b_c");
        assert_eq!(safe_name(".."), "file");
        assert_eq!(safe_name(""), "file");
        assert_eq!(safe_name("invoice\u{202e}fdp.exe"), "invoicefdp.exe");
        assert_eq!(safe_name("a\u{0}b\nc"), "abc");
        assert_eq!(safe_name("报告 2026.pdf"), "报告 2026.pdf");
    }

    #[test]
    fn formats() {
        assert_eq!(bytes(512.0), "512 B");
        assert_eq!(bytes(4300.0), "4.2 KB");
        assert_eq!(bytes(90_000.0), "88 KB");
        assert_eq!(bytes(3_000_000.0), "2.9 MB");
        assert_eq!(time(41.2), "42s");
        assert_eq!(time(187.0), "3m07s");
        assert_eq!(time(f64::NAN), "–");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped(12), "12");
    }
}

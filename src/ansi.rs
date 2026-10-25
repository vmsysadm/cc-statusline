/// Convert hex color codes (#RRGGBB or #RGB) in a string to ANSI escape sequences.
pub(crate) fn convert_color_value(val: &str) -> String {
    if val.is_empty() {
        return val.to_string();
    }
    // Built as bytes so multi-byte UTF-8 passes through untouched.
    let mut result: Vec<u8> = Vec::with_capacity(val.len() * 2);
    let bytes = val.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'#' {
            if i + 6 < bytes.len() && bytes[i + 1..=i + 6].iter().all(|b| b.is_ascii_hexdigit()) {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&val[i + 1..i + 3], 16),
                    u8::from_str_radix(&val[i + 3..i + 5], 16),
                    u8::from_str_radix(&val[i + 5..i + 7], 16),
                ) {
                    result.extend_from_slice(format!("\x1b[38;2;{};{};{}m", r, g, b).as_bytes());
                    i += 7;
                    continue;
                }
            } else if i + 3 < bytes.len() && bytes[i + 1..=i + 3].iter().all(|b| b.is_ascii_hexdigit()) {
                let r_val = u8::from_str_radix(&val[i + 1..i + 2], 16).unwrap_or(0) * 17;
                let g_val = u8::from_str_radix(&val[i + 2..i + 3], 16).unwrap_or(0) * 17;
                let b_val = u8::from_str_radix(&val[i + 3..i + 4], 16).unwrap_or(0) * 17;
                result.extend_from_slice(format!("\x1b[38;2;{};{};{}m", r_val, g_val, b_val).as_bytes());
                i += 4;
                continue;
            }
        }
        result.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&result).into_owned()
}

/// Get the visible column width of a string (ANSI codes excluded) without heap allocation.
pub(crate) fn visible_len(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    let mut width = 0;
    let mut in_esc = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
        } else if in_esc {
            if c == 'm' {
                in_esc = false;
            }
        } else {
            width += UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    width
}

/// Truncate a string to a maximum visible width. Keep ANSI codes intact.
pub(crate) fn truncate_to_visible_width(s: &str, max_width: usize) -> String {
    let mut out = String::with_capacity(s.len());
    let mut cur_width = 0;
    let mut in_esc = false;
    let mut esc_buf = String::new();

    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
            esc_buf.push(c);
            continue;
        }
        if in_esc {
            esc_buf.push(c);
            if c == 'm' {
                in_esc = false;
                out.push_str(&esc_buf);
                esc_buf.clear();
            }
            continue;
        }

        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if cur_width + w > max_width {
            break;
        }
        out.push(c);
        cur_width += w;
    }

    out.push_str("\x1b[0m");
    out
}

/// Format a large number into a human-readable string (K/M suffixes).
pub(crate) fn format_human(num: u64) -> String {
    if num >= 1_000_000 {
        format!("{:.1}M", num as f64 / 1_000_000.0)
    } else if num >= 1_000 {
        format!("{:.1}K", num as f64 / 1_000.0)
    } else {
        num.to_string()
    }
}

/// Format seconds into a compact duration string (e.g. "2d4h", "1h5m", "3m", "45s").
///
/// Days matter here: the 7-day rate-limit window resets up to 168 hours out, and "151h20m" is
/// not something anyone reads at a glance.
pub(crate) fn format_seconds(s: u64) -> String {
    if s == 0 {
        return "0s".to_string();
    }
    if s >= 86_400 {
        let d = s / 86_400;
        let h = (s % 86_400) / 3600;
        format!("{}d{}h", d, h)
    } else if s >= 3600 {
        let h = s / 3600;
        let m = (s % 3600) / 60;
        format!("{}h{}m", h, m)
    } else if s >= 60 {
        let m = s / 60;
        format!("{}m", m)
    } else {
        format!("{}s", s)
    }
}

/// The visible text of a string, ANSI codes removed. For tests that assert on content.
#[cfg(test)]
pub(crate) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
        } else if in_esc {
            if c == 'm' {
                in_esc = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        assert_eq!(strip_ansi("\x1b[1m\x1b[38;2;1;2;3m24%\x1b[0m \x1b[90m5h\x1b[0m"), "24% 5h");
    }

    #[test]
    fn test_convert_color_value() {
        assert_eq!(convert_color_value(""), "");
        assert_eq!(convert_color_value("#ff0000"), "\x1b[38;2;255;0;0m");
        assert_eq!(convert_color_value("#f00"), "\x1b[38;2;255;0;0m");
        assert_eq!(
            convert_color_value("text #5c6370 text"),
            "text \x1b[38;2;92;99;112m text"
        );
    }

    #[test]
    fn test_visible_len() {
        let colored = "\x1b[38;2;255;0;0mHello\x1b[0m";
        assert_eq!(visible_len(colored), 5);
    }

    #[test]
    fn test_format_human() {
        assert_eq!(format_human(500), "500");
        assert_eq!(format_human(1500), "1.5K");
        assert_eq!(format_human(2_500_000), "2.5M");
    }

    #[test]
    fn test_format_seconds() {
        assert_eq!(format_seconds(0), "0s");
        assert_eq!(format_seconds(45), "45s");
        assert_eq!(format_seconds(120), "2m");
        assert_eq!(format_seconds(3660), "1h1m");
        assert_eq!(format_seconds(86_400 + 4 * 3600 + 59), "1d4h");
        assert_eq!(format_seconds(7 * 86_400), "7d0h");
    }

    #[test]
    fn test_unicode_width_visible_len() {
        let emoji_str = "🟢 Ready";
        // 🟢 has column width 2, " Ready" has width 6 => total width 8
        assert_eq!(visible_len(emoji_str), 8);
    }

    #[test]
    fn test_truncate_to_visible_width() {
        let text = "\x1b[31mHello World\x1b[0m";
        let truncated = truncate_to_visible_width(text, 5);
        assert_eq!(visible_len(&truncated), 5);

        let ascii_text = "Standard Text";
        let truncated_ascii = truncate_to_visible_width(ascii_text, 8);
        assert_eq!(visible_len(&truncated_ascii), 8);
    }
}

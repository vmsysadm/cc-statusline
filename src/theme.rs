use std::env;

use crate::ansi::convert_color_value;
use crate::config::Config;

/// Resolved theme holding all ANSI color strings and icon strings.
pub(crate) struct Theme {
    pub(crate) reset: String,
    pub(crate) bold: String,
    pub(crate) italic: String,

    pub(crate) fg_red: String,
    pub(crate) fg_green: String,
    pub(crate) fg_yellow: String,
    pub(crate) fg_blue: String,
    pub(crate) fg_magenta: String,
    pub(crate) fg_cyan: String,
    pub(crate) fg_white: String,
    pub(crate) fg_gray: String,

    pub(crate) fg_bright_red: String,
    pub(crate) fg_bright_yellow: String,
    pub(crate) fg_bright_blue: String,
    pub(crate) fg_bright_cyan: String,
    pub(crate) fg_bright_magenta: String,

    pub(crate) num_color: String,
    pub(crate) dot: String,

    pub(crate) use_ascii: bool,

    // Icons — components
    pub(crate) icon_folder: String,
    pub(crate) icon_model: String,
    pub(crate) icon_branch: String,
    pub(crate) icon_session: String,
    pub(crate) icon_ctx: String,
    pub(crate) icon_tok: String,
    pub(crate) icon_cost: String,
    pub(crate) icon_lines: String,
    pub(crate) icon_duration: String,

    // Icons — permission mode
    pub(crate) icon_yolo: String,
    pub(crate) icon_accept_edits: String,
    pub(crate) icon_plan: String,
    pub(crate) icon_auto: String,
    pub(crate) icon_dont_ask: String,

    // Icons — other
    pub(crate) icon_fast: String,
}

impl Theme {
    /// Build a fully resolved theme from the config.
    pub(crate) fn new(cfg: &Config) -> Self {
        let reset = cfg.colors.as_ref().and_then(|c| c.reset.as_deref()).map(convert_color_value).unwrap_or_else(|| "\x1b[0m".to_string());
        let bold = cfg.colors.as_ref().and_then(|c| c.bold.as_deref()).map(convert_color_value).unwrap_or_else(|| "\x1b[1m".to_string());
        let italic = cfg.colors.as_ref().and_then(|c| c.italic.as_deref()).map(convert_color_value).unwrap_or_else(|| "\x1b[3m".to_string());

        let get_fg = |key: &str, default_val: &str| -> String {
            cfg.colors
                .as_ref()
                .and_then(|c| c.foreground.as_ref())
                .and_then(|fg| fg.get(key))
                .map(|v| convert_color_value(v))
                .unwrap_or_else(|| default_val.to_string())
        };

        let fg_red = get_fg("red", "\x1b[31m");
        let fg_green = get_fg("green", "\x1b[32m");
        let fg_yellow = get_fg("yellow", "\x1b[33m");
        let fg_blue = get_fg("blue", "\x1b[34m");
        let fg_magenta = get_fg("magenta", "\x1b[35m");
        let fg_cyan = get_fg("cyan", "\x1b[36m");
        let fg_white = get_fg("white", "\x1b[37m");
        let fg_gray = get_fg("gray", "\x1b[90m");

        let fg_bright_red = get_fg("bright_red", "\x1b[91m");
        let fg_bright_yellow = get_fg("bright_yellow", "\x1b[93m");
        let fg_bright_blue = get_fg("bright_blue", "\x1b[94m");
        let fg_bright_cyan = get_fg("bright_cyan", "\x1b[96m");
        let fg_bright_magenta = get_fg("bright_magenta", "\x1b[95m");
        let fg_bright_white = get_fg("bright_white", "\x1b[97m");

        let num_color = format!("{}{}", fg_bright_white, bold);
        let dot = cfg.colors
            .as_ref()
            .and_then(|c| c.ui.as_ref())
            .and_then(|ui| ui.get("separator"))
            .map(|v| convert_color_value(v))
            .unwrap_or_else(|| format!("{} | {}", fg_gray, reset));

        let use_ascii = cfg.use_ascii.unwrap_or_else(|| {
            if let Ok(v) = env::var("USE_ASCII") {
                v == "true" || v == "1"
            } else if let Ok(v) = env::var("USE_NERD_FONTS") {
                v == "false" || v == "0"
            } else {
                // No explicit signal: ASCII unless nerd fonts were opted into.
                !cfg.use_nerd_fonts.unwrap_or(false)
            }
        });

        let use_nerd_fonts = !use_ascii
            && cfg.use_nerd_fonts.unwrap_or_else(|| {
                env::var("USE_NERD_FONTS").map(|v| v != "false" && v != "0").unwrap_or(true)
            });

        let cfg_icons = cfg.icons.as_ref().and_then(|i| {
            if use_ascii {
                i.get("ascii").cloned()
            } else if use_nerd_fonts {
                i.get("nerd_fonts").cloned()
            } else {
                i.get("emoji_fallback").cloned()
            }
        });

        // One lookup per icon: (category, key, ascii, nerd font, emoji).
        let icon = |category: &str, key: &str, ascii: &str, nerd: &str, emoji: &str| -> String {
            let default = if use_ascii { ascii } else if use_nerd_fonts { nerd } else { emoji };
            get_icon_str(&cfg_icons, &cfg.icons, category, key, default)
        };

        let cat_comp = "components";
        let cat_mode = "mode";
        let cat_oth = "other";

        let icon_folder = icon(cat_comp, "folder", "DIR:", "\u{ea83}", "📁");
        let icon_model = icon(cat_comp, "model", "", "\u{f0eb}", "💡");
        let icon_branch = icon(cat_comp, "branch", "GIT:", "\u{e725}", "⎇");
        let icon_session = icon(cat_comp, "session", "ID:", "\u{f036a}", "💬");
        let icon_ctx = icon(cat_comp, "context", "CTX:", "\u{f134f}", "📊");
        let icon_tok = icon(cat_comp, "token", "TOK:", "\u{e26b}", "🪙");
        let icon_cost = icon(cat_comp, "cost", "", "", "");
        let icon_lines = icon(cat_comp, "lines", "LOC:", "\u{f044}", "✏");
        let icon_duration = icon(cat_comp, "duration", "TIME:", "\u{f017}", "⏱");

        let icon_yolo = icon(cat_mode, "yolo", "!", "\u{f071}", "⚠");
        let icon_accept_edits = icon(cat_mode, "accept_edits", "[OK]", "\u{f012c}", "✅");
        let icon_plan = icon(cat_mode, "plan", "[PLAN]", "\u{f0349}", "🔍");
        let icon_auto = icon(cat_mode, "auto", "[AUTO]", "\u{f021}", "🔄");
        let icon_dont_ask = icon(cat_mode, "dont_ask", "[NOASK]", "\u{f05e}", "🚫");

        let icon_fast = icon(cat_oth, "fast", "FAST", "\u{f0e7}", "⚡");

        Theme {
            reset, bold, italic,
            fg_red, fg_green, fg_yellow, fg_blue, fg_magenta, fg_cyan, fg_white, fg_gray,
            fg_bright_red, fg_bright_yellow, fg_bright_blue,
            fg_bright_cyan, fg_bright_magenta,
            num_color, dot,
            use_ascii,
            icon_folder, icon_model, icon_branch, icon_session, icon_ctx, icon_tok,
            icon_cost, icon_lines, icon_duration,
            icon_yolo, icon_accept_edits, icon_plan, icon_auto, icon_dont_ask,
            icon_fast,
        }
    }
}

/// Look up an icon string from the icon config, with fallback chains.
pub(crate) fn get_icon_str(
    active_icon_set: &Option<serde_json::Value>,
    raw_icons_cfg: &Option<serde_json::Value>,
    category: &str,
    key: &str,
    default_str: &str,
) -> String {
    // Look in the active icon set (`icons.<variant>.<category>.<key>`), then fall back to a
    // flat `icons.<key>` map, then the built-in default.
    let lookup = |v: &serde_json::Value| -> Option<String> {
        v.get(category)
            .and_then(|cat| cat.get(key))
            .or_else(|| v.get(key))
            .and_then(|s| s.as_str())
            .map(str::to_string)
    };

    active_icon_set
        .as_ref()
        .and_then(lookup)
        .or_else(|| raw_icons_cfg.as_ref().and_then(lookup))
        .unwrap_or_else(|| default_str.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_icon_str_flat_lookup() {
        let active_icons: Option<serde_json::Value> = None;
        let flat_icons: Option<serde_json::Value> = serde_json::from_str(r#"{"plan": "[P]"}"#).ok();
        let icon = get_icon_str(&active_icons, &flat_icons, "mode", "plan", "*");
        assert_eq!(icon, "[P]");
    }

    #[test]
    fn test_icon_set_follows_font_mode() {
        let icons = serde_json::json!({
            "ascii": { "mode": { "yolo": "A!" } },
            "nerd_fonts": { "mode": { "yolo": "N!" } }
        });
        let ascii = Theme::new(&Config { use_ascii: Some(true), icons: Some(icons.clone()), ..Default::default() });
        assert_eq!(ascii.icon_yolo, "A!");
        let nerd = Theme::new(&Config {
            use_ascii: Some(false),
            use_nerd_fonts: Some(true),
            icons: Some(icons),
            ..Default::default()
        });
        assert_eq!(nerd.icon_yolo, "N!");
    }

    /// The configs the installer ships must parse, and their keys must be the ones looked up
    /// here -- a renamed key would otherwise fall back to the built-in default without a trace.
    #[test]
    fn test_shipped_configs() {
        let nerd: Config = serde_json::from_str(include_str!("../statusline_config.json")).unwrap();
        let t = Theme::new(&nerd);
        assert!(!t.use_ascii);
        assert_eq!(t.icon_session, "\u{f036a}");
        assert_eq!(t.icon_dont_ask, "\u{f05e}");
        // The warning triangle agy's shipped config uses, not the flame its code defaults to.
        assert_eq!(t.icon_yolo, "\u{f071}");
        // No icon: the amount's own `$` says what it is.
        assert_eq!(t.icon_cost, "");
        assert_eq!(Theme::new(&Config { use_ascii: Some(false), use_nerd_fonts: Some(true), ..Default::default() }).icon_yolo, "\u{f071}");
        assert_eq!(t.fg_gray, "\x1b[38;2;92;99;112m");

        let ascii: Config = serde_json::from_str(include_str!("../ascii_statusline_config.json")).unwrap();
        let t = Theme::new(&ascii);
        assert!(t.use_ascii);
        assert_eq!(t.icon_model, "MOD:");
        assert_eq!(t.icon_plan, "[PLAN]");
    }

    /// Private-use codepoints DejaVu Sans fills with glyphs of its own.
    ///
    /// When the terminal font is not a Nerd Font, icons are drawn from a fallback font, and
    /// fontconfig ranks DejaVu (installed nearly everywhere on Linux) ahead of the Nerd Font. An
    /// icon in one of these ranges therefore renders as DejaVu's unrelated mark -- which is how
    /// the Octicons branch (U+F418) and light bulb (U+F400) inherited from agy came out blank.
    const DEJAVU_PUA: [(u32, u32); 4] = [(0xEF00, 0xEF19), (0xF000, 0xF003), (0xF400, 0xF441), (0xF6C5, 0xF6C5)];

    #[test]
    fn test_nerd_icons_avoid_dejavu_private_use() {
        let shipped: Config = serde_json::from_str(include_str!("../statusline_config.json")).unwrap();
        let builtin = Config { use_ascii: Some(false), use_nerd_fonts: Some(true), ..Default::default() };
        for cfg in [shipped, builtin] {
            let t = Theme::new(&cfg);
            let icons = [
                ("folder", &t.icon_folder), ("model", &t.icon_model), ("branch", &t.icon_branch),
                ("session", &t.icon_session), ("context", &t.icon_ctx), ("token", &t.icon_tok),
                ("cost", &t.icon_cost), ("lines", &t.icon_lines), ("duration", &t.icon_duration),
                ("yolo", &t.icon_yolo), ("accept_edits", &t.icon_accept_edits), ("plan", &t.icon_plan),
                ("auto", &t.icon_auto), ("dont_ask", &t.icon_dont_ask), ("fast", &t.icon_fast),
            ];
            for (name, icon) in icons {
                for c in icon.chars() {
                    let cp = c as u32;
                    assert!(
                        !DEJAVU_PUA.iter().any(|(lo, hi)| (*lo..=*hi).contains(&cp)),
                        "{} icon U+{:04X} is shadowed by DejaVu Sans",
                        name,
                        cp
                    );
                }
            }
        }
    }
}

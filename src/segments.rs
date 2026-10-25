use std::fs;
use std::path::{Path, PathBuf};

use crate::ansi::{format_human, format_seconds};
use crate::data::{ContextWindow, InputData, RateWindow};
use crate::mode::BYPASS;
use crate::theme::Theme;

/// Everything besides the payload that rendering depends on, resolved once by `main`.
pub(crate) struct RenderEnv<'a> {
    pub(crate) home_path: &'a str,
    /// Resolved permission mode; `None` is the default mode.
    pub(crate) mode: Option<&'a str>,
    /// Terminal width, from the `COLUMNS` Claude Code exports to the command.
    pub(crate) cols: usize,
    /// Wall clock in Unix seconds, injected so rate-limit countdowns are testable.
    pub(crate) now: u64,
}

/// Parse the contents of a git `HEAD` file into a branch name, or a short hash when detached.
fn parse_head(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if let Some(r) = trimmed.strip_prefix("ref: ") {
        let r = r.trim();
        return Some(r.strip_prefix("refs/heads/").unwrap_or(r).to_string());
    }
    if trimmed.len() >= 7 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(trimmed[..7].to_string());
    }
    None
}

/// Find the git directory for `cwd`, walking up to the repository root.
///
/// A `.git` *file* -- a linked worktree or a submodule -- names the real git directory on its
/// `gitdir:` line, relative to the file itself unless absolute.
fn find_git_dir(cwd: &str) -> Option<PathBuf> {
    let mut dir = Some(Path::new(cwd));
    while let Some(d) = dir {
        let dot_git = d.join(".git");
        if let Ok(meta) = fs::metadata(&dot_git) {
            if meta.is_dir() {
                return Some(dot_git);
            }
            let content = fs::read_to_string(&dot_git).ok()?;
            let target = content.lines().find_map(|l| l.strip_prefix("gitdir:"))?.trim();
            return Some(d.join(target));
        }
        dir = d.parent();
    }
    None
}

/// Read the current git branch for a working directory.
///
/// Reads `HEAD` directly instead of spawning git: the status line runs on every refresh, and a
/// process spawn costs more than the whole render.
pub(crate) fn get_git_branch(cwd: &str) -> Option<String> {
    if cwd.is_empty() {
        return None;
    }
    let git_dir = find_git_dir(cwd)?;
    parse_head(&fs::read_to_string(git_dir.join("HEAD")).ok()?)
}

/// The part of `path` below `home`, or `None` when `path` is not inside it.
///
/// Matching on whole components: `/home/user` must not claim `/home/username`.
fn strip_home<'a>(path: &'a str, home: &str) -> Option<&'a str> {
    let home = home.trim_end_matches(['/', '\\']);
    if home.is_empty() {
        return None;
    }
    let rest = path.strip_prefix(home)?;
    (rest.is_empty() || rest.starts_with(['/', '\\'])).then_some(rest)
}

/// Shorten a path for display, with home-directory replacement.
pub(crate) fn get_shortened_path(path: &str, max_len: usize, home_path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let short_path = match strip_home(path, home_path) {
        Some(rest) => format!("~{}", rest),
        None => path.to_string(),
    };

    let leaf = || {
        Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string()
    };

    if max_len == 0 {
        if short_path == "~" {
            "~".to_string()
        } else {
            leaf()
        }
    } else if short_path.chars().count() > max_len {
        format!("...{}", leaf())
    } else {
        short_path
    }
}

/// Format a git branch name with icon and color.
pub(crate) fn format_branch(branch: Option<&str>, max_len: usize, fg_blue: &str, icon_branch: &str, reset: &str) -> String {
    if let Some(name) = branch {
        let mut b_name = name.to_string();
        if max_len > 0 && b_name.chars().count() > max_len {
            b_name = format!("{}..", b_name.chars().take(max_len).collect::<String>());
        }
        format!("{}{}{} {}{}", fg_blue, icon_branch, reset, b_name, reset)
    } else {
        String::new()
    }
}

/// Prefix a segment body with its icon, or leave the body bare when the icon set has none.
fn labeled(color: &str, icon: &str, body: &str, t: &Theme) -> String {
    if icon.is_empty() {
        body.to_string()
    } else {
        format!("{}{}{} {}", color, icon, t.reset, body)
    }
}

/// Format the permission-mode badge. The default mode shows nothing.
pub(crate) fn format_mode(mode: Option<&str>, t: &Theme) -> String {
    let mode = match mode {
        Some(m) if m != "default" => m,
        _ => return String::new(),
    };
    let (color, icon, label) = match mode {
        BYPASS => (&t.fg_bright_red, t.icon_yolo.as_str(), "YOLO".to_string()),
        "acceptEdits" => (&t.fg_bright_yellow, t.icon_accept_edits.as_str(), "ACCEPT-EDITS".to_string()),
        "plan" => (&t.fg_bright_blue, t.icon_plan.as_str(), "PLAN".to_string()),
        "auto" => (&t.fg_bright_cyan, t.icon_auto.as_str(), "AUTO".to_string()),
        "dontAsk" => (&t.fg_bright_magenta, t.icon_dont_ask.as_str(), "DONT-ASK".to_string()),
        other => (&t.fg_white, "", other.to_uppercase()),
    };
    if icon.is_empty() {
        format!("{}{}{}{}", color, t.bold, label, t.reset)
    } else {
        format!("{}{}{} {}{}", color, t.bold, icon, label, t.reset)
    }
}

/// Build a progress bar string (unicode block or ASCII).
pub(crate) fn make_bar(pct: f64, len: usize, fill_color: &str, fg_gray: &str, reset: &str, use_ascii: bool) -> String {
    let pct_int = pct.clamp(0.0, 100.0);
    let mut filled = ((pct_int * len as f64) / 100.0).round() as usize;
    if pct_int > 0.0 && filled == 0 {
        filled = 1;
    }

    if use_ascii {
        let empty = len.saturating_sub(filled);
        let mut bar = String::with_capacity(len + fill_color.len() + reset.len() + 2);
        bar.push('[');
        bar.push_str(fill_color);
        for _ in 0..filled {
            bar.push('=');
        }
        bar.push_str(reset);
        for _ in 0..empty {
            bar.push(' ');
        }
        bar.push(']');
        bar
    } else {
        let remainder = ((pct_int * len as f64) % 100.0).floor() as usize;

        let block_full = '\u{2588}';
        let block_dark = '\u{2593}';
        let block_med = '\u{2592}';
        let block_light = '\u{2591}';

        let mut bar = String::with_capacity(len * 16);
        for i in 0..len {
            if i < filled {
                bar.push_str(fill_color);
                bar.push(block_full);
                bar.push_str(reset);
            } else if i == filled {
                let partial_block = if remainder >= 75 {
                    block_dark
                } else if remainder >= 50 {
                    block_med
                } else {
                    block_light
                };
                bar.push_str(fill_color);
                bar.push(partial_block);
                bar.push_str(reset);
                bar.push_str(fg_gray);
            } else {
                bar.push_str(fg_gray);
                bar.push(block_light);
                bar.push_str(reset);
            }
        }
        bar
    }
}

/// One rate-limit window, ready to display.
#[derive(Clone, Debug)]
pub(crate) struct Limit {
    pub(crate) label: &'static str,
    pub(crate) used: f64,
    pub(crate) resets_in: Option<u64>,
    /// `(used, limit)` in dollars, when a spend limit reports them.
    pub(crate) dollars: Option<(f64, f64)>,
}

/// Short label for a spend-limit period.
fn period_label(period: Option<&str>) -> &'static str {
    match period {
        Some("monthly") => "mo",
        Some("weekly") => "wk",
        Some("daily") => "day",
        _ => "spend",
    }
}

/// Gather the rate-limit windows Claude Code reported, in display order.
///
/// Each window is independently optional, and Claude Code drops one once its `resets_at` has
/// passed, so an absent window is simply not shown.
pub(crate) fn collect_limits(data: &InputData, now: u64) -> Vec<Limit> {
    let rl = match &data.rate_limits {
        Some(rl) => rl,
        None => return Vec::new(),
    };
    let windows: [(&Option<RateWindow>, Option<&'static str>); 3] = [
        (&rl.five_hour, Some("5h")),
        (&rl.seven_day, Some("7d")),
        (&rl.spend_limit, None),
    ];

    windows
        .iter()
        .filter_map(|(w, label)| {
            let w = w.as_ref()?;
            let used = w.used_percentage?;
            Some(Limit {
                label: label.unwrap_or_else(|| period_label(w.period.as_deref())),
                used,
                resets_in: w.resets_at.map(|at| (at as u64).saturating_sub(now)),
                dollars: w.used_usd.zip(w.limit_usd),
            })
        })
        .collect()
}

/// Format a single rate-limit window for display.
pub(crate) fn format_single_limit(l: &Limit, mode: &str, t: &Theme) -> String {
    let (fg_gray, num_color, reset) = (&t.fg_gray, &t.num_color, &t.reset);
    let value = match l.dollars {
        Some((used, limit)) => format!("${:.0}/${:.0}", used, limit),
        None => format!("{}%", l.used.round() as u64),
    };

    if mode == "narrow" {
        return format!("{}{}{} {}{}{}", num_color, value, reset, fg_gray, l.label, reset);
    }

    let countdown = l
        .resets_in
        .map(|s| format!(" {}{}{}", fg_gray, format_seconds(s), reset))
        .unwrap_or_default();
    let body = format!("{}{}{} {}{}{}{}", num_color, value, reset, fg_gray, l.label, reset, countdown);

    if mode == "med" {
        return body;
    }

    let fill = if l.used >= 90.0 {
        &t.fg_bright_red
    } else if l.used >= 60.0 {
        &t.fg_bright_cyan
    } else {
        &t.fg_cyan
    };
    format!("{}{}", make_bar(l.used, 5, fill, fg_gray, reset, t.use_ascii), body)
}

/// Format all rate-limit windows. Narrow shows only the window closest to its limit.
pub(crate) fn format_limits(limits: &[Limit], mode: &str, t: &Theme) -> String {
    if mode == "narrow" {
        return limits
            .iter()
            .max_by(|a, b| a.used.partial_cmp(&b.used).unwrap_or(std::cmp::Ordering::Equal))
            .map(|l| format_single_limit(l, mode, t))
            .unwrap_or_default();
    }
    limits
        .iter()
        .map(|l| format_single_limit(l, mode, t))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Context-window usage, as a percentage.
///
/// Claude Code rounds `used_percentage` to a whole number; recomputing it from the same
/// input-only numerator (`total_input_tokens` already sums fresh input with cache reads and
/// writes) keeps the decimal the bar displays honest.
pub(crate) fn context_used_pct(ctx: &ContextWindow) -> f64 {
    let in_tok = ctx.total_input_tokens.unwrap_or(0.0);
    let limit = ctx.context_window_size.unwrap_or(0.0);
    if in_tok > 0.0 && limit > 0.0 {
        in_tok / limit * 100.0
    } else {
        ctx.used_percentage.unwrap_or(0.0)
    }
}

/// Build all segment strings for a given density and theme.
pub(crate) struct Segments {
    // Permission mode
    pub(crate) mode_seg: String,
    // Model — three densities
    pub(crate) m_wide: String,
    pub(crate) m_med: String,
    pub(crate) m_narrow: String,
    // Directory — three densities
    pub(crate) dir_wide: String,
    pub(crate) dir_med: String,
    pub(crate) dir_narrow: String,
    // Branch — three densities
    pub(crate) v_wide: String,
    pub(crate) v_med: String,
    pub(crate) v_narrow: String,
    // Session — two densities
    pub(crate) sess_wide: String,
    pub(crate) sess_med: String,
    // Context bar — three densities
    pub(crate) ctx_bar_wide: String,
    pub(crate) ctx_bar_med: String,
    pub(crate) ctx_bar_narrow: String,
    // Token details
    pub(crate) tok_details_wide: String,
    pub(crate) tok_details_med: String,
    // Session totals: cost, lines changed (two densities), wall time
    pub(crate) cost_seg: String,
    pub(crate) lines_wide: String,
    pub(crate) lines_narrow: String,
    pub(crate) dur_seg: String,
    // Rate limits — three densities
    pub(crate) limits_wide: String,
    pub(crate) limits_med: String,
    pub(crate) limits_narrow: String,
}

impl Segments {
    /// Build all segment strings from input data and theme.
    pub(crate) fn new(data: &InputData, t: &Theme, env: &RenderEnv) -> Self {
        let mode_seg = format_mode(env.mode, t);

        // Model
        let model_info = data.model.as_ref();
        let model_id = model_info.and_then(|m| m.id.as_deref()).unwrap_or("");
        let model_name = model_info
            .and_then(|m| m.display_name.as_deref())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| model_id.trim_start_matches("claude-"));
        let effort = data.effort.as_ref().and_then(|e| e.level.as_deref()).unwrap_or("");

        let model_effort = if !effort.is_empty() {
            format!("{} ({})", model_name, effort)
        } else {
            model_name.to_string()
        };

        let model_seg = |label: &str| -> String {
            if label.is_empty() {
                String::new()
            } else if t.icon_model.is_empty() {
                format!("{}{}{}{}", t.fg_bright_magenta, t.italic, label, t.reset)
            } else {
                format!("{}{}{} {}{}", t.fg_bright_magenta, t.italic, t.icon_model, label, t.reset)
            }
        };

        // Fast mode bills at a premium, so it rides along with the model wherever there is room.
        let fast = if data.fast_mode == Some(true) && !model_name.is_empty() {
            format!(" {}{}{}{}", t.fg_bright_yellow, t.bold, t.icon_fast, t.reset)
        } else {
            String::new()
        };

        let m_wide = format!("{}{}", model_seg(&model_effort), fast);
        let m_med = m_wide.clone();
        let m_narrow = model_seg(&model_name.chars().take(12).collect::<String>());

        // Directory
        let cwd = data.current_dir();
        let dir_seg = |max_len: usize| -> String {
            let val = get_shortened_path(cwd, max_len, env.home_path);
            if val.is_empty() {
                String::new()
            } else {
                format!("{}{}{} {}{}", t.fg_cyan, t.icon_folder, t.reset, val, t.reset)
            }
        };
        let dir_wide = dir_seg(25);
        let dir_med = dir_seg(15);
        let dir_narrow = dir_seg(0);

        // Branch: a Claude Code worktree session names its branch; otherwise read HEAD.
        let git_branch = data
            .worktree
            .as_ref()
            .and_then(|w| w.branch.clone())
            .or_else(|| get_git_branch(cwd));
        let v_wide = format_branch(git_branch.as_deref(), 15, &t.fg_blue, &t.icon_branch, &t.reset);
        let v_med = format_branch(git_branch.as_deref(), 10, &t.fg_blue, &t.icon_branch, &t.reset);
        let v_narrow = format_branch(git_branch.as_deref(), 6, &t.fg_blue, &t.icon_branch, &t.reset);

        // Session
        let session_id = data.session_id.as_deref().unwrap_or("");
        let sess_seg = |len: usize| -> String {
            if session_id.is_empty() {
                String::new()
            } else {
                let short: String = session_id.chars().take(len).collect();
                format!("{}{}{} {}{}", t.fg_gray, t.icon_session, t.reset, short, t.reset)
            }
        };
        let sess_wide = sess_seg(8);
        let sess_med = sess_seg(4);

        // Context bar
        let ctx_default = ContextWindow::default();
        let ctx = data.context_window.as_ref().unwrap_or(&ctx_default);
        let in_tok = ctx.total_input_tokens.unwrap_or(0.0) as u64;
        let out_tok = ctx.total_output_tokens.unwrap_or(0.0) as u64;
        let limit = ctx.context_window_size.unwrap_or(0.0) as u64;
        let used_pct = context_used_pct(ctx);
        let pct_int = used_pct as u64;
        let fill_color = if pct_int >= 90 {
            &t.fg_bright_red
        } else if pct_int >= 60 {
            &t.fg_bright_yellow
        } else {
            &t.fg_yellow
        };

        let bar_wide = make_bar(used_pct, 15, fill_color, &t.fg_gray, &t.reset, t.use_ascii);
        let bar_med = make_bar(used_pct, 10, fill_color, &t.fg_gray, &t.reset, t.use_ascii);
        let bar_narrow = make_bar(used_pct, 6, fill_color, &t.fg_gray, &t.reset, t.use_ascii);

        let ctx_fraction = if in_tok > 0 || limit > 0 {
            format!(" ({}/{})", format_human(in_tok), format_human(limit))
        } else {
            String::new()
        };

        let ctx_bar_wide = format!("{}{}{} {}{}{:.1}%{}{}", t.fg_yellow, t.icon_ctx, t.reset, bar_wide, t.num_color, used_pct, t.reset, ctx_fraction);
        let ctx_bar_med = format!("{}{}{} {}{}{:.1}%{}{}", t.fg_yellow, t.icon_ctx, t.reset, bar_med, t.num_color, used_pct, t.reset, ctx_fraction);
        let ctx_bar_narrow = format!("{}{}{} {}{}{}%{}", t.fg_yellow, t.icon_ctx, t.reset, bar_narrow, t.num_color, pct_int, t.reset);

        // Token details: the last API call, broken out by category.
        let (in_display, out_display, cache_read, cache_write) = match &ctx.current_usage {
            Some(cur) => (
                cur.input_tokens.unwrap_or(0.0) as u64,
                cur.output_tokens.unwrap_or(0.0) as u64,
                cur.cache_read_input_tokens.unwrap_or(0.0) as u64,
                cur.cache_creation_input_tokens.unwrap_or(0.0) as u64,
            ),
            None => (in_tok, out_tok, 0, 0),
        };

        let mut tok_breakdown_wide = format!("({} in/{} out", format_human(in_display), format_human(out_display));
        if cache_read > 0 {
            tok_breakdown_wide.push_str(&format!("/{} cache", format_human(cache_read)));
        }
        if cache_write > 0 {
            tok_breakdown_wide.push_str(&format!("/{} write", format_human(cache_write)));
        }
        tok_breakdown_wide.push(')');

        // Prompt cache: a cold cache means the next request re-sends the whole prefix at full
        // price, which is worth knowing before typing it. `warm: false` alone is not cold --
        // with caching unobserved there is no cache to go cold.
        let cache_note = match &data.prompt_cache {
            Some(pc) if pc.caching_observed == Some(true) => {
                if pc.warm == Some(false) {
                    format!(" {}cold{}", t.fg_bright_blue, t.reset)
                } else if let Some(ratio) = pc.hit_ratio {
                    format!(" {}{}% hit{}", t.fg_gray, (ratio * 100.0).round() as u64, t.reset)
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        };

        let tok_breakdown_med = format!("({} in/{} out)", format_human(in_display), format_human(out_display));

        let tok_details_wide = if in_display > 0 || out_display > 0 || cache_read > 0 || cache_write > 0 {
            format!("{}{}{} {}{}", t.fg_yellow, t.icon_tok, t.reset, tok_breakdown_wide, cache_note)
        } else {
            String::new()
        };

        let tok_details_med = if in_display > 0 || out_display > 0 {
            format!("{}{}{} {}", t.fg_yellow, t.icon_tok, t.reset, tok_breakdown_med)
        } else {
            String::new()
        };

        // Session totals
        let (cost_seg, lines_wide, lines_narrow, dur_seg) = match &data.cost {
            Some(cost) => {
                let usd = cost.total_cost_usd.unwrap_or(0.0);
                let added = cost.total_lines_added.unwrap_or(0.0) as u64;
                let removed = cost.total_lines_removed.unwrap_or(0.0) as u64;
                let lines = format!("{}+{}{} {}-{}{}", t.fg_green, added, t.reset, t.fg_red, removed, t.reset);
                let secs = (cost.total_duration_ms.unwrap_or(0.0) / 1000.0) as u64;
                (
                    labeled(&t.fg_green, &t.icon_cost, &format!("{}${:.2}{}", t.num_color, usd, t.reset), t),
                    labeled(&t.fg_blue, &t.icon_lines, &lines, t),
                    lines,
                    labeled(&t.fg_magenta, &t.icon_duration, &format!("{}{}{}", t.num_color, format_seconds(secs), t.reset), t),
                )
            }
            None => Default::default(),
        };

        // Rate limits
        let limits = collect_limits(data, env.now);
        let limits_wide = format_limits(&limits, "wide", t);
        let limits_med = format_limits(&limits, "med", t);
        let limits_narrow = format_limits(&limits, "narrow", t);

        Segments {
            mode_seg,
            m_wide, m_med, m_narrow,
            dir_wide, dir_med, dir_narrow,
            v_wide, v_med, v_narrow,
            sess_wide, sess_med,
            ctx_bar_wide, ctx_bar_med, ctx_bar_narrow,
            tok_details_wide, tok_details_med,
            cost_seg, lines_wide, lines_narrow, dur_seg,
            limits_wide, limits_med, limits_narrow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansi::{strip_ansi, visible_len};
    use crate::config::Config;

    fn ascii_theme() -> Theme {
        Theme::new(&Config { use_ascii: Some(true), ..Default::default() })
    }

    fn render_env<'a>(home: &'a str, mode: Option<&'a str>) -> RenderEnv<'a> {
        RenderEnv { home_path: home, mode, cols: 160, now: 1_738_420_000 }
    }

    fn full() -> InputData {
        serde_json::from_str(include_str!("../tests/fixtures/full.json")).unwrap()
    }

    /// A scratch directory removed on drop.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(label: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("cc-statusline-{}-{}", label, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    #[test]
    fn test_make_bar() {
        let bar = make_bar(100.0, 5, "", "", "", false);
        assert_eq!(visible_len(&bar), 5);
        let bar_ascii = make_bar(50.0, 6, "", "", "", true);
        assert_eq!(visible_len(&bar_ascii), 8); // [===   ]
        assert_eq!(bar_ascii, "[===   ]");
        let bar_ascii_low = make_bar(4.2, 15, "", "", "", true);
        assert_eq!(bar_ascii_low, "[=              ]");
    }

    /// A stand-in home directory, built from the platform's own temp location so the test carries
    /// no assumption about where homes live.
    fn fake_home() -> PathBuf {
        std::env::temp_dir().join("cc-home")
    }

    #[test]
    fn test_get_shortened_path() {
        let home = fake_home();
        let cwd = home.join("project");
        let sep = std::path::MAIN_SEPARATOR;

        assert_eq!(
            get_shortened_path(&cwd.to_string_lossy(), 20, &home.to_string_lossy()),
            format!("~{}project", sep)
        );
        assert_eq!(get_shortened_path(&home.to_string_lossy(), 0, &home.to_string_lossy()), "~");

        // A path outside the home directory keeps its own shape.
        let outside = std::env::temp_dir().join("elsewhere").join("project");
        assert_eq!(
            get_shortened_path(&outside.to_string_lossy(), 0, &home.to_string_lossy()),
            "project"
        );
    }

    #[test]
    fn test_home_prefix_needs_a_component_boundary() {
        assert_eq!(get_shortened_path("/home/username/x", 40, "/home/user"), "/home/username/x");
        assert_eq!(get_shortened_path("/home/user/x", 40, "/home/user/"), "~/x");
        assert_eq!(get_shortened_path("/srv/x", 40, "/"), "/srv/x");
    }

    #[test]
    fn test_parse_head() {
        assert_eq!(parse_head("ref: refs/heads/main\n").as_deref(), Some("main"));
        assert_eq!(parse_head("ref: refs/heads/feat/x").as_deref(), Some("feat/x"));
        assert_eq!(parse_head("0d2056ecf42d9452a8cd8e054600024e7c557add\n").as_deref(), Some("0d2056e"));
        assert_eq!(parse_head("garbage"), None);
    }

    #[test]
    fn test_git_branch_from_subdirectory() {
        let root = scratch("git-sub");
        fs::create_dir_all(root.0.join(".git")).unwrap();
        fs::write(root.0.join(".git").join("HEAD"), "ref: refs/heads/trunk\n").unwrap();
        let sub = root.0.join("src").join("deep");
        fs::create_dir_all(&sub).unwrap();

        assert_eq!(get_git_branch(&sub.to_string_lossy()).as_deref(), Some("trunk"));
    }

    #[test]
    fn test_git_branch_in_linked_worktree() {
        // `git worktree add` leaves a `.git` file pointing at the per-worktree git dir.
        let root = scratch("git-wt");
        let real = root.0.join("main-repo").join(".git").join("worktrees").join("wt");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
        let wt = root.0.join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();

        assert_eq!(get_git_branch(&wt.to_string_lossy()).as_deref(), Some("feature"));

        // A relative gitdir resolves against the directory holding the `.git` file.
        fs::write(wt.join(".git"), "gitdir: ../main-repo/.git/worktrees/wt\n").unwrap();
        assert_eq!(get_git_branch(&wt.to_string_lossy()).as_deref(), Some("feature"));
    }

    #[test]
    fn test_worktree_payload_branch_wins() {
        let mut data = full();
        data.worktree = Some(crate::data::Worktree { branch: Some("worktree-x".to_string()) });
        let t = ascii_theme();
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.v_wide.contains("worktree-x"));
    }

    #[test]
    fn test_context_pct_recomputed_from_tokens() {
        let data = InputData {
            context_window: Some(ContextWindow {
                total_input_tokens: Some(126936.0),
                total_output_tokens: Some(62398.0),
                context_window_size: Some(250000.0),
                used_percentage: Some(51.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let t = ascii_theme();
        let segs = Segments::new(&data, &t, &render_env(&fake_home().to_string_lossy(), None));
        assert!(segs.ctx_bar_wide.contains("50.8%"), "{}", segs.ctx_bar_wide);
        assert!(segs.ctx_bar_wide.contains("126.9K/250.0K"));
    }

    #[test]
    fn test_context_pct_falls_back_to_payload() {
        let ctx = ContextWindow { used_percentage: Some(42.0), ..Default::default() };
        assert_eq!(context_used_pct(&ctx), 42.0);
        assert_eq!(context_used_pct(&ContextWindow::default()), 0.0);
    }

    #[test]
    fn test_token_breakdown() {
        let t = ascii_theme();
        let segs = Segments::new(&full(), &t, &render_env("/home/dev", None));
        assert!(segs.ctx_bar_wide.contains("12.8%"), "{}", segs.ctx_bar_wide);
        assert!(segs.ctx_bar_wide.contains("128.2K/1.0M"));
        assert!(segs.tok_details_wide.contains("5.6K in"));
        assert!(segs.tok_details_wide.contains("1.2K out"));
        assert!(segs.tok_details_wide.contains("117.8K cache"));
        assert!(segs.tok_details_wide.contains("4.8K write"));
        assert!(segs.tok_details_wide.contains("91% hit"));
        assert!(!segs.tok_details_med.contains("cache"));
    }

    #[test]
    fn test_cold_cache() {
        let mut data = full();
        let pc = data.prompt_cache.as_mut().unwrap();
        pc.warm = Some(false);
        let t = ascii_theme();
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.tok_details_wide.contains("cold"));

        // No caching observed: nothing to be cold.
        data.prompt_cache.as_mut().unwrap().caching_observed = Some(false);
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(!segs.tok_details_wide.contains("cold"));
        assert!(!segs.tok_details_wide.contains("hit"));
    }

    #[test]
    fn test_session_start_has_no_token_details() {
        let data: InputData = serde_json::from_str(include_str!("../tests/fixtures/session_start.json")).unwrap();
        let t = ascii_theme();
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.tok_details_wide.is_empty());
        assert!(segs.ctx_bar_wide.contains("0.0%"));
        assert!(segs.limits_wide.is_empty());
        assert!(segs.cost_seg.contains("$0.00"));
    }

    #[test]
    fn test_model_segment() {
        let t = ascii_theme();
        let mut data = full();
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.m_wide.contains("Opus 5.5 (xhigh)"));
        assert!(!segs.m_wide.contains("FAST"));
        assert!(segs.m_narrow.contains("Opus 5.5") && !segs.m_narrow.contains("xhigh"));

        data.fast_mode = Some(true);
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.m_wide.contains("FAST"));

        // No display name: fall back to the id, minus the vendor prefix.
        data.model = Some(crate::data::Model { id: Some("claude-sonnet-5-5".to_string()), display_name: None });
        data.effort = None;
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.m_med.contains("sonnet-5-5"));
        assert!(!segs.m_med.contains("claude-"));
    }

    #[test]
    fn test_mode_badge() {
        let t = ascii_theme();
        assert_eq!(format_mode(None, &t), "");
        assert_eq!(format_mode(Some("default"), &t), "");
        assert!(format_mode(Some(BYPASS), &t).contains("! YOLO"));
        assert!(format_mode(Some("plan"), &t).contains("[PLAN] PLAN"));
        assert!(format_mode(Some("acceptEdits"), &t).contains("ACCEPT-EDITS"));
        assert!(format_mode(Some("auto"), &t).contains("AUTO"));
        assert!(format_mode(Some("dontAsk"), &t).contains("DONT-ASK"));
        // A mode this build does not know still shows up rather than vanishing.
        assert!(format_mode(Some("newMode"), &t).contains("NEWMODE"));
    }

    #[test]
    fn test_session_totals() {
        let t = ascii_theme();
        let segs = Segments::new(&full(), &t, &render_env("/home/dev", None));
        assert!(segs.cost_seg.contains("$1.23"));
        assert!(segs.lines_wide.contains("+156") && segs.lines_wide.contains("-23"));
        assert!(segs.dur_seg.contains("45m"), "{}", segs.dur_seg);

        let data = InputData::default();
        let segs = Segments::new(&data, &t, &render_env("/home/dev", None));
        assert!(segs.cost_seg.is_empty() && segs.lines_wide.is_empty() && segs.dur_seg.is_empty());
    }

    #[test]
    fn test_rate_limits() {
        let data = full();
        let limits = collect_limits(&data, 1_738_420_000);
        let labels: Vec<_> = limits.iter().map(|l| l.label).collect();
        assert_eq!(labels, ["5h", "7d", "mo"]);
        assert_eq!(limits[0].resets_in, Some(5600));
        assert_eq!(limits[2].dollars, Some((314.12, 500.0)));

        let t = ascii_theme();
        let wide = strip_ansi(&format_limits(&limits, "wide", &t));
        assert!(wide.contains("24% 5h 1h33m"), "{}", wide);
        assert!(wide.contains("41% 7d 5d1h"), "{}", wide);
        assert!(wide.contains("$314/$500 mo"), "{}", wide);

        // Narrow keeps only the window nearest its limit, without its countdown.
        let narrow = strip_ansi(&format_limits(&limits, "narrow", &t));
        assert_eq!(narrow, "$314/$500 mo");

        // A reset already in the past counts down to zero rather than wrapping.
        assert_eq!(collect_limits(&data, 1_800_000_000)[0].resets_in, Some(0));
    }

    #[test]
    fn test_spend_limit_without_dollars() {
        let data: InputData = serde_json::from_str(
            r#"{"rate_limits": {"spend_limit": {"used_percentage": 101.4, "resets_at": 1}}}"#,
        )
        .unwrap();
        let limits = collect_limits(&data, 0);
        assert_eq!(limits.len(), 1);
        let t = ascii_theme();
        assert_eq!(strip_ansi(&format_limits(&limits, "med", &t)), "101% spend 1s");
    }
}

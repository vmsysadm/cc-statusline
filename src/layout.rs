use crate::ansi::{truncate_to_visible_width, visible_len};
use crate::data::InputData;
use crate::segments::{RenderEnv, Segments};
use crate::theme::Theme;

/// Columns Claude Code keeps for itself around the status row: it indents the row by two and
/// cuts it off with `…` two short of the right edge (measured against v2.1.310). A line any
/// wider than `COLUMNS - CC_MARGIN` loses its tail.
const CC_MARGIN: usize = 4;

/// Join non-empty items with a separator.
fn join_non_empty(items: &[&str], sep: &str) -> String {
    items.iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(sep)
}

/// Right-align two segments within a given column width.
fn print_right_aligned(left: &str, right: &str, total_cols: usize) -> String {
    if right.is_empty() {
        return left.to_string();
    }
    let left_vis = visible_len(left);
    let right_vis = visible_len(right);

    let pad = if total_cols > left_vis + right_vis {
        total_cols - left_vis - right_vis
    } else {
        1
    };

    format!("{}{}{}", left, " ".repeat(pad), right)
}

fn pad_to_visible_len(s: &str, target_len: usize) -> String {
    format!("{}{}", s, " ".repeat(target_len.saturating_sub(visible_len(s))))
}

/// Build the final statusline output (1 or 2 lines).
pub(crate) fn render_statusline(data: &InputData, t: &Theme, env: &RenderEnv) -> Vec<String> {
    let s = Segments::new(data, t, env);

    let dot = &t.dot;

    let line1_wide = join_non_empty(&[&s.mode_seg, &s.m_wide, &s.ctx_bar_wide, &s.dir_wide, &s.v_wide, &s.sess_wide, &s.tok_details_wide], dot);
    let line2_wide = join_non_empty(&[&s.cost_seg, &s.lines_wide, &s.dur_seg, &s.limits_wide], dot);

    let cols = env.cols;
    let target_cols = cols.saturating_sub(CC_MARGIN);
    let margin = 8;

    let len1_wide = visible_len(&line1_wide);
    let len2_wide = visible_len(&line2_wide);

    let mut output_lines = Vec::new();

    if cols >= 135 && target_cols >= (len1_wide + len2_wide + margin) {
        output_lines.push(print_right_aligned(&line1_wide, &line2_wide, target_cols));
    }

    if output_lines.is_empty() {
        let r_candidate = |idx: usize| -> ((String, String), (String, String)) {
            let (p1, r1_right, ctx_seg) = match idx {
                0 => (
                    join_non_empty(&[&s.mode_seg, &s.m_wide], dot),
                    join_non_empty(&[&s.cost_seg, &s.lines_wide, &s.dur_seg], dot),
                    s.ctx_bar_wide.as_str(),
                ),
                1 => (
                    join_non_empty(&[&s.mode_seg, &s.m_wide], dot),
                    join_non_empty(&[&s.cost_seg, &s.lines_wide, &s.dur_seg], dot),
                    s.ctx_bar_med.as_str(),
                ),
                2 => (
                    join_non_empty(&[&s.mode_seg, &s.m_med], dot),
                    join_non_empty(&[&s.cost_seg, &s.lines_wide], dot),
                    s.ctx_bar_med.as_str(),
                ),
                3 => (
                    join_non_empty(&[&s.mode_seg, &s.m_med], dot),
                    join_non_empty(&[&s.cost_seg, &s.lines_narrow], "  "),
                    s.ctx_bar_narrow.as_str(),
                ),
                4 => (
                    join_non_empty(&[&s.mode_seg, &s.m_narrow], dot),
                    s.cost_seg.clone(),
                    s.ctx_bar_narrow.as_str(),
                ),
                _ => (
                    join_non_empty(&[&s.mode_seg, &s.m_narrow], dot),
                    String::new(),
                    "",
                ),
            };

            let (p2, r2_right, tok_seg) = match idx {
                0 => (
                    join_non_empty(&[&s.dir_wide, &s.v_wide, &s.sess_wide], dot),
                    s.limits_wide.as_str(),
                    s.tok_details_wide.as_str(),
                ),
                1 => (
                    join_non_empty(&[&s.dir_wide, &s.v_wide, &s.sess_wide], dot),
                    s.limits_wide.as_str(),
                    s.tok_details_med.as_str(),
                ),
                2 => (
                    join_non_empty(&[&s.dir_wide, &s.v_wide, &s.sess_wide], dot),
                    s.limits_med.as_str(),
                    s.tok_details_med.as_str(),
                ),
                3 => (
                    join_non_empty(&[&s.dir_med, &s.v_med, &s.sess_med], dot),
                    s.limits_med.as_str(),
                    s.tok_details_med.as_str(),
                ),
                4 => (
                    join_non_empty(&[&s.dir_med, &s.v_med, &s.sess_med], dot),
                    s.limits_med.as_str(),
                    "",
                ),
                5 => (
                    join_non_empty(&[&s.dir_narrow, &s.v_narrow], dot),
                    s.limits_med.as_str(),
                    "",
                ),
                6 => (
                    join_non_empty(&[&s.dir_narrow, &s.v_narrow], dot),
                    s.limits_narrow.as_str(),
                    "",
                ),
                7 => (
                    join_non_empty(&[&s.dir_narrow], dot),
                    s.limits_narrow.as_str(),
                    "",
                ),
                8 => (
                    join_non_empty(&[&s.dir_narrow], dot),
                    "",
                    "",
                ),
                _ => (
                    String::new(),
                    "",
                    "",
                ),
            };

            let p1_len = visible_len(&p1);
            let p2_len = visible_len(&p2);

            let (r1_left, r2_left) = if !ctx_seg.is_empty() && !tok_seg.is_empty() && p1_len > 0 && p2_len > 0 {
                let max_p = p1_len.max(p2_len);
                let p1_pad = pad_to_visible_len(&p1, max_p);
                let p2_pad = pad_to_visible_len(&p2, max_p);
                (
                    format!("{}{}{}", p1_pad, dot, ctx_seg),
                    format!("{}{}{}", p2_pad, dot, tok_seg),
                )
            } else {
                (
                    if ctx_seg.is_empty() { p1 } else if p1.is_empty() { ctx_seg.to_string() } else { format!("{}{}{}", p1, dot, ctx_seg) },
                    if tok_seg.is_empty() { p2 } else if p2.is_empty() { tok_seg.to_string() } else { format!("{}{}{}", p2, dot, tok_seg) },
                )
            };

            ((r1_left, r1_right), (r2_left, r2_right.to_string()))
        };

        // Candidate 9 is the last resort (mode and model only, nothing right-aligned); anything
        // still too wide after that is handled by the truncate below.
        let ((r1_left, r1_right), (r2_left, r2_right)) = (0..9)
            .map(r_candidate)
            .find(|((l1, r1), (l2, r2))| {
                let l1_vis = visible_len(l1);
                let r1_vis = visible_len(r1);
                let req1 = if r1_vis == 0 { l1_vis } else { l1_vis + 1 + r1_vis };

                let l2_vis = visible_len(l2);
                let r2_vis = visible_len(r2);
                let req2 = if r2_vis == 0 { l2_vis } else { l2_vis + 1 + r2_vis };

                req1 <= target_cols && req2 <= target_cols
            })
            .unwrap_or_else(|| r_candidate(9));

        let mut line1_str = print_right_aligned(&r1_left, &r1_right, target_cols);
        if visible_len(&line1_str) > target_cols {
            line1_str = truncate_to_visible_width(&line1_str, target_cols);
        }

        let mut line2_str = print_right_aligned(&r2_left, &r2_right, target_cols);
        if visible_len(&line2_str) > target_cols {
            line2_str = truncate_to_visible_width(&line2_str, target_cols);
        }

        output_lines.push(line1_str);
        output_lines.push(line2_str);
    }

    // Claude Code shows every printed line as a row, blank or not.
    output_lines.retain(|l| visible_len(l.trim()) > 0);
    output_lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansi::{strip_ansi, visible_len};
    use crate::config::Config;

    fn full() -> InputData {
        serde_json::from_str(include_str!("../tests/fixtures/full.json")).unwrap()
    }

    fn render(data: &InputData, cfg: &Config, cols: usize, mode: Option<&str>) -> Vec<String> {
        let t = Theme::new(cfg);
        let env = RenderEnv { home_path: "/home/dev", mode, cols, now: 1_738_420_000 };
        render_statusline(data, &t, &env)
    }

    #[test]
    fn test_dynamic_line_width_truncation_across_widths() {
        let widths = [30, 50, 80, 100, 115, 135, 160, 220];
        let configs = [
            Config { use_ascii: Some(true), ..Default::default() },
            Config { use_ascii: Some(false), use_nerd_fonts: Some(true), ..Default::default() },
            Config { use_ascii: Some(false), use_nerd_fonts: Some(false), ..Default::default() },
        ];

        for cfg in &configs {
            for &cols in &widths {
                let lines = render(&full(), cfg, cols, Some("bypassPermissions"));
                assert!(!lines.is_empty());
                assert!(lines.len() <= 2);

                let target_cols = cols.saturating_sub(CC_MARGIN);
                for line in &lines {
                    let vis = visible_len(line);
                    assert!(
                        vis <= target_cols,
                        "Line visible width {} exceeds target_cols {} at total width {}: {:?}",
                        vis,
                        target_cols,
                        cols,
                        strip_ansi(line)
                    );
                }
            }
        }
    }

    #[test]
    fn test_wide_terminal_uses_one_line() {
        let cfg = Config { use_ascii: Some(true), ..Default::default() };
        let lines = render(&full(), &cfg, 320, Some("bypassPermissions"));
        assert_eq!(lines.len(), 1);
        let text = strip_ansi(&lines[0]);
        assert!(text.starts_with("! YOLO"), "{}", text);
        assert!(text.contains("$1.23") && text.contains("5h"), "{}", text);
    }

    #[test]
    fn test_two_line_split() {
        let cfg = Config { use_ascii: Some(true), ..Default::default() };
        let lines = render(&full(), &cfg, 120, None);
        assert_eq!(lines.len(), 2);
        let (l1, l2) = (strip_ansi(&lines[0]), strip_ansi(&lines[1]));
        assert!(l1.contains("Opus 5.5") && l1.contains("CTX:") && l1.contains("$1.23"), "{}", l1);
        assert!(l2.contains("DIR:") && l2.contains("TOK:") && l2.contains("5h"), "{}", l2);
        // The default permission mode has no badge.
        assert!(!l1.contains("YOLO"));
    }

    #[test]
    fn test_no_blank_rows() {
        let cfg = Config { use_ascii: Some(true), ..Default::default() };
        for cols in [40, 100, 200] {
            let lines = render(&InputData::default(), &cfg, cols, None);
            assert!(!lines.is_empty());
            for line in &lines {
                assert!(!strip_ansi(line).trim().is_empty(), "blank row at {} cols", cols);
            }
        }
    }
}

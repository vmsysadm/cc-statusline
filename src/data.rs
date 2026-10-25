//! The JSON payload Claude Code pipes to the status line command on stdin.
//!
//! Field names follow the documented schema (https://code.claude.com/docs/en/statusline).
//! Every number is read as `f64`: the payload is produced by JavaScript, where integers and
//! floats are one type, so a count that is integral today may not be tomorrow.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};

/// Deserialize a field, degrading to `None` when it has an unexpected shape.
///
/// The payload is versioned by the CLI, not by us. A field that changes type in a future Claude
/// Code release must cost that one segment, not blank the whole status line.
fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).ok())
}

#[derive(Deserialize, Debug, Default, Clone)]
pub(crate) struct CurrentUsage {
    #[serde(default)]
    pub(crate) input_tokens: Option<f64>,
    #[serde(default)]
    pub(crate) output_tokens: Option<f64>,
    #[serde(default)]
    pub(crate) cache_creation_input_tokens: Option<f64>,
    #[serde(default)]
    pub(crate) cache_read_input_tokens: Option<f64>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct ContextWindow {
    /// Everything currently in the window on the input side: fresh input plus cache reads and
    /// writes. This is the numerator of `used_percentage`.
    #[serde(default)]
    pub(crate) total_input_tokens: Option<f64>,
    #[serde(default)]
    pub(crate) total_output_tokens: Option<f64>,
    #[serde(default)]
    pub(crate) context_window_size: Option<f64>,
    /// Rounded to a whole number by Claude Code, and `null` early in a session.
    #[serde(default)]
    pub(crate) used_percentage: Option<f64>,
    /// `null` before the first API call and again right after `/compact`.
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) current_usage: Option<CurrentUsage>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Model {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) display_name: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Workspace {
    #[serde(default)]
    pub(crate) current_dir: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Cost {
    #[serde(default)]
    pub(crate) total_cost_usd: Option<f64>,
    #[serde(default)]
    pub(crate) total_duration_ms: Option<f64>,
    #[serde(default)]
    pub(crate) total_lines_added: Option<f64>,
    #[serde(default)]
    pub(crate) total_lines_removed: Option<f64>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct PromptCache {
    /// Whether the cached prefix is still inside its TTL.
    #[serde(default)]
    pub(crate) warm: Option<bool>,
    /// False when caching is off or the provider does not report it -- `warm: false` then
    /// means "no cache", not "cache expired".
    #[serde(default)]
    pub(crate) caching_observed: Option<bool>,
    #[serde(default)]
    pub(crate) hit_ratio: Option<f64>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Effort {
    #[serde(default)]
    pub(crate) level: Option<String>,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub(crate) struct RateWindow {
    /// 0-100; a spend limit can run past 100.
    #[serde(default)]
    pub(crate) used_percentage: Option<f64>,
    /// Unix epoch seconds.
    #[serde(default)]
    pub(crate) resets_at: Option<f64>,
    /// Spend-limit only, and only some of the time: the dollar figures arrive on a slower
    /// schedule than the percentage.
    #[serde(default)]
    pub(crate) used_usd: Option<f64>,
    #[serde(default)]
    pub(crate) limit_usd: Option<f64>,
    #[serde(default)]
    pub(crate) period: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct RateLimits {
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) five_hour: Option<RateWindow>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) seven_day: Option<RateWindow>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) spend_limit: Option<RateWindow>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct Worktree {
    #[serde(default)]
    pub(crate) branch: Option<String>,
}

#[derive(Deserialize, Debug, Default)]
pub(crate) struct InputData {
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) session_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) transcript_path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) cwd: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) model: Option<Model>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) workspace: Option<Workspace>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) cost: Option<Cost>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) context_window: Option<ContextWindow>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) prompt_cache: Option<PromptCache>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) fast_mode: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) effort: Option<Effort>,
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) rate_limits: Option<RateLimits>,
    /// Present only while the session is in a Claude Code worktree session.
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) worktree: Option<Worktree>,
    /// Not sent as of Claude Code 2.1.310 -- the status line is re-run when the mode changes,
    /// but the mode itself is left out. Honoured first if a later release adds it.
    #[serde(default, deserialize_with = "lenient")]
    pub(crate) permission_mode: Option<String>,
}

impl InputData {
    /// The live working directory. The schema sends it twice; `workspace.current_dir` is the
    /// documented preference.
    pub(crate) fn current_dir(&self) -> &str {
        self.workspace
            .as_ref()
            .and_then(|w| w.current_dir.as_deref())
            .or(self.cwd.as_deref())
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_payload_parses() {
        let data: InputData =
            serde_json::from_str(include_str!("../tests/fixtures/full.json")).unwrap();
        assert_eq!(data.model.as_ref().unwrap().display_name.as_deref(), Some("Opus 5.5"));
        assert_eq!(data.effort.unwrap().level.as_deref(), Some("xhigh"));
        let limits = data.rate_limits.unwrap();
        assert_eq!(limits.five_hour.unwrap().used_percentage, Some(23.5));
        assert_eq!(limits.spend_limit.unwrap().limit_usd, Some(500.0));
        assert_eq!(data.cost.unwrap().total_lines_added, Some(156.0));
    }

    #[test]
    fn test_session_start_payload_parses() {
        // The first refresh of a session: no API response yet, so `current_usage` and
        // `used_percentage` are null and `rate_limits` / `prompt_cache` are absent.
        let data: InputData =
            serde_json::from_str(include_str!("../tests/fixtures/session_start.json")).unwrap();
        let ctx = data.context_window.unwrap();
        assert!(ctx.current_usage.is_none());
        assert!(ctx.used_percentage.is_none());
        assert!(data.rate_limits.is_none());
        assert!(data.prompt_cache.is_none());
    }

    #[test]
    fn test_mistyped_field_costs_only_itself() {
        let data: InputData = serde_json::from_str(
            r#"{"model": "not-an-object", "effort": {"level": 3}, "cwd": "/tmp/x"}"#,
        )
        .unwrap();
        assert!(data.model.is_none());
        assert!(data.effort.is_none());
        assert_eq!(data.current_dir(), "/tmp/x");
    }

    #[test]
    fn test_current_dir_prefers_workspace() {
        let data: InputData = serde_json::from_str(
            r#"{"cwd": "/a", "workspace": {"current_dir": "/b"}}"#,
        )
        .unwrap();
        assert_eq!(data.current_dir(), "/b");
    }
}

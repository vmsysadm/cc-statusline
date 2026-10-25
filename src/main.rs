mod ansi;
mod config;
mod data;
mod layout;
mod mode;
mod segments;
mod theme;

use std::env;
use std::io::{self, BufWriter, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

/// Fallback width for a run outside Claude Code, which always exports `COLUMNS`.
const DEFAULT_COLS: usize = 80;

/// Read an environment variable, treating empty as unset.
fn env_nonempty(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}

fn main() {
    let mut input = String::new();
    let _ = io::stdin().read_to_string(&mut input);

    // Garbage in degrades to a default line rather than an error: Claude Code blanks the status
    // line on a non-zero exit.
    let data: data::InputData = serde_json::from_str(&input).unwrap_or_default();

    let home_path = env_nonempty("HOME").or_else(|| env_nonempty("USERPROFILE")).unwrap_or_default();
    let cfg_path = config::config_path(
        &home_path,
        env_nonempty("CLAUDE_CONFIG_DIR").as_deref(),
        env_nonempty("CC_STATUSLINE_CONFIG").as_deref(),
    );
    let cfg = config::load_config(&cfg_path);

    let mode = mode::resolve(&data, env_nonempty("CLAUDE_PID").as_deref(), cfg.yolo.unwrap_or(false));

    // Claude Code captures the command's output, so the terminal cannot be asked for its size;
    // it exports the dimensions instead.
    let cols = env_nonempty("COLUMNS").and_then(|c| c.parse().ok()).unwrap_or(DEFAULT_COLS);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

    let theme = theme::Theme::new(&cfg);
    let render_env = segments::RenderEnv { home_path: &home_path, mode: mode.as_deref(), cols, now };
    let output_lines = layout::render_statusline(&data, &theme, &render_env);

    let stdout = io::stdout();
    let mut handle = BufWriter::new(stdout.lock());
    for line in output_lines {
        let _ = writeln!(handle, "{}", line);
    }
}

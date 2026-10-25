//! Black-box tests that drive the built binary.
//!
//! The crate is binary-only, so `tests/` cannot import its modules -- but Cargo still exports
//! `CARGO_BIN_EXE_cc-statusline`, which is enough to test the one layer the in-crate unit tests
//! structurally cannot reach: `main`'s own wiring of the environment Claude Code hands the
//! command (`HOME`, `COLUMNS`, `CLAUDE_PID`, `CLAUDE_CONFIG_DIR`). Env vars are process-global
//! and Rust runs tests on threads, so a child process with an environment of its own is the
//! only honest way to test that.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const EXE: &str = env!("CARGO_BIN_EXE_cc-statusline");

/// Variables the binary reads. Every run starts with all of them cleared, so the developer's
/// own session -- which may well be a Claude Code session exporting `CLAUDE_PID` -- cannot leak
/// into the result.
const READ_VARS: [&str; 8] = [
    "HOME",
    "USERPROFILE",
    "COLUMNS",
    "CLAUDE_PID",
    "CLAUDE_CONFIG_DIR",
    "CC_STATUSLINE_CONFIG",
    "USE_ASCII",
    "USE_NERD_FONTS",
];

/// Run the binary on `payload` with only the given variables set. Returns the output lines with
/// ANSI codes stripped.
fn run(payload: &str, env: &[(&str, &str)]) -> Vec<String> {
    let mut cmd = Command::new(EXE);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for k in READ_VARS {
        cmd.env_remove(k);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("failed to spawn statusline binary");
    child.stdin.as_mut().unwrap().write_all(payload.as_bytes()).unwrap();

    let out = child.wait_with_output().unwrap();
    // Claude Code blanks the status line on a non-zero exit, so this is the contract.
    assert!(out.status.success(), "binary exited with {:?}", out.status);
    assert!(out.stderr.is_empty(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    let text = String::from_utf8(out.stdout).expect("output was not utf-8");
    text.lines().map(strip_ansi).collect()
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
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

fn width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// A scratch directory, unique per test.
fn temp_dir_named(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-statusline-cli-{}-{}", label, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const FULL: &str = include_str!("fixtures/full.json");
const SESSION_START: &str = include_str!("fixtures/session_start.json");

const ASCII: (&str, &str) = ("USE_ASCII", "true");

#[test]
fn fits_inside_claude_codes_margins_at_every_width() {
    for cols in [24, 40, 60, 80, 100, 120, 135, 160, 200, 300] {
        let cols_s = cols.to_string();
        for payload in [FULL, SESSION_START] {
            for fonts in [("USE_ASCII", "true"), ("USE_NERD_FONTS", "true"), ("USE_NERD_FONTS", "false")] {
                let lines = run(payload, &[("COLUMNS", &cols_s), fonts]);
                assert!(!lines.is_empty() && lines.len() <= 2, "{} lines at {}", lines.len(), cols);
                for line in &lines {
                    assert!(
                        width(line) <= cols - 4,
                        "{} cols wide at COLUMNS={}: {:?}",
                        width(line),
                        cols,
                        line
                    );
                }
            }
        }
    }
}

#[test]
fn columns_drives_the_layout() {
    let narrow = run(FULL, &[("COLUMNS", "100"), ASCII]);
    let wide = run(FULL, &[("COLUMNS", "320"), ASCII]);
    assert_eq!(narrow.len(), 2);
    assert_eq!(wide.len(), 1);
}

#[test]
fn missing_columns_falls_back_to_80() {
    for line in run(FULL, &[ASCII]) {
        assert!(width(&line) <= 76, "{:?}", line);
    }
}

/// A payload whose working directory is the home directory itself, which renders as `~` only if
/// `main` resolved the same directory as home.
#[test]
fn home_comes_from_home_var() {
    let home = temp_dir_named("home");
    let home = home.to_str().unwrap();
    let payload = serde_json::json!({ "cwd": home }).to_string();

    let lines = run(&payload, &[("HOME", home), ("COLUMNS", "120"), ASCII]);
    assert!(lines.iter().any(|l| l.contains("DIR: ~")), "{:?}", lines);
}

#[test]
fn missing_home_vars_do_not_panic() {
    let payload = serde_json::json!({ "cwd": "/srv/project" }).to_string();
    let lines = run(&payload, &[("COLUMNS", "120"), ASCII]);
    assert!(lines.iter().any(|l| l.contains("DIR: /srv/project")), "{:?}", lines);
}

/// The CLI pipes whatever it has; garbage must degrade to a default line, never an error.
#[test]
fn malformed_stdin_still_renders() {
    for payload in ["", "not json at all", "{\"cwd\":", "null", "[]", "{\"model\": 5}"] {
        let lines = run(payload, &[ASCII]);
        assert!(!lines.is_empty(), "no output for payload {:?}", payload);
    }
}

#[test]
fn config_follows_claude_config_dir() {
    let dir = temp_dir_named("config-dir");
    let cfg_dir = dir.join("cc-statusline");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::write(
        cfg_dir.join("statusline_config.json"),
        r#"{"use_ascii": true, "colors": {"ui": {"separator": " // "}}}"#,
    )
    .unwrap();

    let lines = run(FULL, &[("CLAUDE_CONFIG_DIR", dir.to_str().unwrap()), ("COLUMNS", "120")]);
    assert!(lines[0].contains(" // "), "custom separator not applied: {:?}", lines);
}

#[test]
fn explicit_config_path_wins() {
    let dir = temp_dir_named("explicit-config");
    let path = dir.join("mine.json");
    std::fs::write(&path, r#"{"use_ascii": true, "yolo": true}"#).unwrap();

    let lines = run(FULL, &[("CC_STATUSLINE_CONFIG", path.to_str().unwrap()), ("COLUMNS", "120")]);
    assert!(lines[0].starts_with("! YOLO"), "{:?}", lines);
}

#[test]
fn mode_comes_from_the_transcript() {
    let dir = temp_dir_named("transcript");
    let transcript = dir.join("session.jsonl");
    std::fs::write(
        &transcript,
        concat!(
            r#"{"type":"permission-mode","permissionMode":"default","sessionId":"s"}"#, "\n",
            r#"{"type":"user","message":{"role":"user","content":"go"},"permissionMode":"plan"}"#, "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"}]}}"#, "\n",
        ),
    )
    .unwrap();
    let payload = serde_json::json!({
        "transcript_path": transcript,
        "model": {"display_name": "Opus 5.5"},
    })
    .to_string();

    let lines = run(&payload, &[("COLUMNS", "120"), ASCII]);
    assert!(lines[0].starts_with("[PLAN] PLAN"), "{:?}", lines);
}

/// `CLAUDE_PID` names the Claude Code process; its launch flags decide the mode before the
/// transcript has recorded one. A stand-in process carrying the flag is as real as it gets.
#[cfg(target_os = "linux")]
#[test]
fn mode_comes_from_the_claude_process_flags() {
    // `sleep` is not the script's last command, so the shell cannot exec it in place and swap
    // out the command line under test.
    let mut stand_in = Command::new("sh")
        .args(["-c", "sleep 30; :", "claude", "--dangerously-skip-permissions"])
        .spawn()
        .unwrap();
    let pid = stand_in.id().to_string();

    let payload = serde_json::json!({ "model": {"display_name": "Opus 5.5"} }).to_string();
    let with_flag = run(&payload, &[("CLAUDE_PID", &pid), ("COLUMNS", "120"), ASCII]);
    let without = run(&payload, &[("COLUMNS", "120"), ASCII]);

    let _ = stand_in.kill();
    let _ = stand_in.wait();

    assert!(with_flag[0].starts_with("! YOLO"), "{:?}", with_flag);
    assert!(!without[0].contains("YOLO"), "{:?}", without);
}

/// The one thing the original agy statusline did that this port must not: Claude Code draws its
/// own tab indicators, so nothing here may write a terminal title or any other OSC sequence.
#[test]
fn emits_no_terminal_title_sequences() {
    let mut cmd = Command::new(EXE);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).env("COLUMNS", "200").env_remove("CLAUDE_PID");
    let mut child = cmd.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(FULL.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(!out.stdout.windows(2).any(|w| w == b"\x1b]"), "OSC sequence in output");
}

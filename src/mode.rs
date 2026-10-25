//! Which permission mode the session is in.
//!
//! Claude Code re-runs the status line whenever the permission mode changes, but as of v2.1.310
//! it leaves the mode out of the payload. Three places can still answer, tried in order:
//!
//! 1. `permission_mode` in the payload, should a later release add it.
//! 2. The transcript. User prompts carry the mode they were submitted under, and Claude Code
//!    periodically re-appends a `{"type":"permission-mode",...}` metadata line. Either way the
//!    newest occurrence is the newest mode the session has *recorded* -- which trails a
//!    shift+tab made while idle until the next turn writes it out.
//! 3. The command line of the Claude Code process (`CLAUDE_PID`), for a session that has not
//!    written anything yet: `--dangerously-skip-permissions` or `--permission-mode <mode>`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::data::InputData;

/// Permission mode value that skips every permission prompt.
pub(crate) const BYPASS: &str = "bypassPermissions";

/// The key as it appears *structurally* in a transcript line. Inside a JSON string the quotes
/// would be escaped (`\"permissionMode\":\"`), so prose or a tool result that merely quotes the
/// key can never match -- only a real field does.
const KEY: &[u8] = b"\"permissionMode\":\"";

/// Read the transcript backwards in chunks of this size.
const CHUNK: u64 = 64 * 1024;

/// Give up after scanning this much of the transcript tail. A single tool result can be half a
/// megabyte, so the last mode record is not always near the end; but a session that has gone
/// this far without one has nothing useful to say, and the scan runs on every refresh.
const MAX_SCAN: u64 = 4 * 1024 * 1024;

/// Longest mode value worth reading. Real values are short camelCase words.
const MAX_VALUE: usize = 32;

/// Pull the mode value out of the bytes that follow [`KEY`].
fn parse_value(after: &[u8]) -> Option<String> {
    let end = memchr::memchr(b'"', &after[..after.len().min(MAX_VALUE + 1)])?;
    let value = &after[..end];
    if value.is_empty() || !value.iter().all(u8::is_ascii_alphabetic) {
        return None;
    }
    Some(String::from_utf8_lossy(value).into_owned())
}

/// The newest permission mode recorded in a transcript.
pub(crate) fn from_transcript(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let floor = len.saturating_sub(MAX_SCAN);

    // Consecutive chunks overlap by enough to hold a whole key and value, so a record that
    // straddles a chunk boundary is still seen in one piece by the earlier chunk.
    let overlap = (KEY.len() + MAX_VALUE + 1) as u64;
    let mut end = len;
    let mut buf = Vec::new();

    while end > floor {
        let start = end.saturating_sub(CHUNK).max(floor);
        let read_end = (end + overlap).min(len);
        buf.resize((read_end - start) as usize, 0);
        file.seek(SeekFrom::Start(start)).ok()?;
        file.read_exact(&mut buf).ok()?;

        let mut hay = &buf[..];
        while let Some(pos) = memchr::memmem::rfind(hay, KEY) {
            if let Some(mode) = parse_value(&hay[pos + KEY.len()..]) {
                return Some(mode);
            }
            hay = &hay[..pos];
        }
        end = start;
    }
    None
}

/// The permission mode a Claude Code command line starts the session in, if it names one.
pub(crate) fn from_cmdline(args: &[String]) -> Option<String> {
    let mut named = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        // Only the exact flag: `--allow-dangerously-skip-permissions` makes bypass reachable
        // through shift+tab but does not start the session in it.
        if arg == "--dangerously-skip-permissions" {
            return Some(BYPASS.to_string());
        }
        if arg == "--permission-mode" {
            named = iter.next().cloned();
        } else if let Some(v) = arg.strip_prefix("--permission-mode=") {
            named = Some(v.to_string());
        }
    }
    named.filter(|m| !m.is_empty())
}

/// Arguments of a running process, from `/proc/<pid>/cmdline`.
#[cfg(target_os = "linux")]
fn process_args(pid: &str) -> Option<Vec<String>> {
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let raw = std::fs::read(format!("/proc/{}/cmdline", pid)).ok()?;
    Some(
        raw.split(|b| *b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .collect(),
    )
}

#[cfg(not(target_os = "linux"))]
fn process_args(_pid: &str) -> Option<Vec<String>> {
    None
}

/// Resolve the session's permission mode. `None` means nothing recorded one -- the default.
///
/// `claude_pid` is the `CLAUDE_PID` Claude Code exports to the status line command.
pub(crate) fn resolve(data: &InputData, claude_pid: Option<&str>, force_yolo: bool) -> Option<String> {
    if force_yolo {
        return Some(BYPASS.to_string());
    }
    data.permission_mode
        .clone()
        .filter(|m| !m.is_empty())
        .or_else(|| data.transcript_path.as_deref().and_then(|p| from_transcript(Path::new(p))))
        .or_else(|| claude_pid.and_then(process_args).and_then(|a| from_cmdline(&a)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempFile(std::path::PathBuf);

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn transcript(label: &str, lines: &[String]) -> TempFile {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "cc-statusline-mode-{}-{}-{}.jsonl",
            label,
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let mut f = File::create(&path).unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        TempFile(path)
    }

    fn meta(mode: &str) -> String {
        format!(r#"{{"type":"permission-mode","permissionMode":"{}","sessionId":"s"}}"#, mode)
    }

    fn prompt(mode: &str) -> String {
        format!(
            r#"{{"parentUuid":null,"type":"user","message":{{"role":"user","content":"hi"}},"permissionMode":"{}"}}"#,
            mode
        )
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_newest_record_wins() {
        let t = transcript("newest", &[meta("default"), prompt("plan"), meta("acceptEdits")]);
        assert_eq!(from_transcript(&t.0).as_deref(), Some("acceptEdits"));

        let t = transcript("prompt-last", &[meta("default"), prompt("plan")]);
        assert_eq!(from_transcript(&t.0).as_deref(), Some("plan"));
    }

    #[test]
    fn test_quoted_key_is_not_a_record() {
        // An assistant reply or tool result quoting the key carries it escaped, inside a string.
        let quoted = r#"{"type":"assistant","message":{"content":"set \"permissionMode\":\"bypassPermissions\" to skip"}}"#;
        let t = transcript("quoted", &[meta("plan"), quoted.to_string()]);
        assert_eq!(from_transcript(&t.0).as_deref(), Some("plan"));
    }

    #[test]
    fn test_record_far_from_the_tail() {
        // One huge tool result after the record: several chunks back, and straddling a boundary.
        let filler = format!(r#"{{"type":"user","toolUseResult":"{}"}}"#, "x".repeat(3 * CHUNK as usize + 17));
        let t = transcript("far", &[meta("auto"), filler]);
        assert_eq!(from_transcript(&t.0).as_deref(), Some("auto"));
    }

    #[test]
    fn test_record_straddling_every_offset() {
        // Slide the record across the first chunk boundary one byte at a time.
        let record = meta("plan");
        for shift in 0..(record.len() + 2) {
            let pad = CHUNK as usize - shift;
            let filler = format!(r#"{{"pad":"{}"}}"#, "y".repeat(pad));
            let t = transcript("straddle", &[record.clone(), filler]);
            assert_eq!(from_transcript(&t.0).as_deref(), Some("plan"), "shift {}", shift);
        }
    }

    #[test]
    fn test_no_record() {
        let t = transcript("none", &[r#"{"type":"user","message":"hi"}"#.to_string()]);
        assert_eq!(from_transcript(&t.0), None);
        assert_eq!(from_transcript(Path::new("/nonexistent/cc-statusline.jsonl")), None);
    }

    #[test]
    fn test_from_cmdline() {
        assert_eq!(from_cmdline(&args(&["claude", "--dangerously-skip-permissions"])).as_deref(), Some(BYPASS));
        assert_eq!(from_cmdline(&args(&["claude", "--permission-mode", "plan"])).as_deref(), Some("plan"));
        assert_eq!(from_cmdline(&args(&["claude", "--permission-mode=acceptEdits"])).as_deref(), Some("acceptEdits"));
        // Skip-permissions wins over a named mode, in either order.
        assert_eq!(
            from_cmdline(&args(&["claude", "--permission-mode", "plan", "--dangerously-skip-permissions"])).as_deref(),
            Some(BYPASS)
        );
        // Merely *allowing* bypass does not enter it.
        assert_eq!(from_cmdline(&args(&["claude", "--allow-dangerously-skip-permissions"])), None);
        assert_eq!(from_cmdline(&args(&["claude", "--permission-mode"])), None);
        assert_eq!(from_cmdline(&args(&["claude"])), None);
        // argv[0] is the program, never a flag.
        assert_eq!(from_cmdline(&args(&["--dangerously-skip-permissions"])), None);
    }

    #[test]
    fn test_resolve_precedence() {
        let t = transcript("resolve", &[meta("plan")]);
        let mut data = InputData::default();
        data.transcript_path = Some(t.0.to_string_lossy().into_owned());
        assert_eq!(resolve(&data, None, false).as_deref(), Some("plan"));

        data.permission_mode = Some("acceptEdits".to_string());
        assert_eq!(resolve(&data, None, false).as_deref(), Some("acceptEdits"));

        assert_eq!(resolve(&data, None, true).as_deref(), Some(BYPASS));
        assert_eq!(resolve(&InputData::default(), None, false), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_resolve_reads_launch_flags() {
        let own = std::process::id().to_string();
        // This test binary was not started with a permission flag.
        assert_eq!(resolve(&InputData::default(), Some(&own), false), None);
        assert!(process_args(&own).is_some_and(|a| !a.is_empty()));
        assert_eq!(process_args("../self"), None);
    }
}

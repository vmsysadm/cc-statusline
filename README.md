# cc-statusline

A responsive, high-performance Rust status line for **Claude Code**, adapted from
agy-statusline ([agy-statusline-win](https://github.com/vmsysadm/agy-statusline-win)), the
Antigravity CLI status line.

```text
! YOLO | MOD: Opus 5.5 (xhigh)               | CTX: [==             ]12.8% (128.2K/1.0M)                         $1.23 | LOC: +156 -23 | TIME: 45m
DIR: ~/my-project | GIT: main | ID: 3f9c2a71 | TOK: (5.6K in/1.2K out/117.8K cache/4.8K write) 91% hit       [=    ]24% 5h 2h5m [==   ]41% 7d 3d4h
```

That is the ASCII theme at 150 columns. The default theme draws the same thing with Nerd Font
icons and One Dark colors, and the layout reflows to fit the terminal:

```text
--- 100 columns
! YOLO | MOD: Opus 5.5 (xhigh)           | CTX: [=     ]12%                      $1.23  +156 -23
DIR: ~/my-project | GIT: main | ID: 3f9c | TOK: (5.6K in/1.2K out)       24% 5h 2h5m 41% 7d 3d4h
--- 60 columns
! YOLO | MOD: Opus 5.5
DIR: my-project | GIT: main      24% 5h 2h5m 41% 7d 3d4h
```

On a very wide terminal everything goes on one line.

## Install

```bash
./install.sh            # builds with cargo, installs, and points Claude Code at it
./install.sh --refresh 0   # same, without the 30-second idle refresh
```

The installer puts the binary and its config in `~/.claude/cc-statusline/` (it follows
`$CLAUDE_CONFIG_DIR` if you relocated Claude Code's config) and sets `statusLine` in
`~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/home/you/.claude/cc-statusline/cc-statusline",
    "padding": 0,
    "refreshInterval": 30
  }
}
```

Claude Code picks the change up live. Any `statusLine` you had before is saved and put back by
`./uninstall.sh` (add `--purge` to delete the config too).

`refreshInterval` keeps the rate-limit countdowns and session timer moving while the session is
idle; without it the line only redraws on Claude Code's own events (a new assistant message, a
permission-mode change, `/compact`, ...).

## What it shows

| Segment | Source |
| --- | --- |
| `YOLO` / `PLAN` / `ACCEPT-EDITS` / `AUTO` / `DONT-ASK` | permission mode -- see below; the default mode shows nothing |
| model and effort, plus a `FAST` marker | `model.display_name`, `effort.level`, `fast_mode` |
| context bar, % and `used/size` | `context_window` (percentage recomputed from tokens, since Claude Code rounds it) |
| directory, git branch | `workspace.current_dir`; branch from `worktree.branch` or the repo's `HEAD` |
| session id | first 8 characters of `session_id` |
| tokens of the last API call | `context_window.current_usage`: input, output, cache reads, cache writes |
| prompt cache | `prompt_cache.hit_ratio`, or `cold` once the cached prefix has expired |
| cost, lines changed, wall time | `cost.*` |
| rate limits with reset countdowns | `rate_limits.five_hour`, `seven_day`, `spend_limit` (dollars when the gateway reports them) |

Rate limits show **used** percentage, as `/usage` does (the agy original showed remaining quota).

## How it differs from the agy original

Claude Code already draws its own tab indicators, spinner and agent panel, so everything the agy
version did to recreate those is gone:

- **No terminal title / OSC output.** The tab title, the pulse animation and the `⚙`/`🤖` tab
  indicators are removed; the binary writes only status text to stdout. A test enforces this.
- **No agent-state segment** (READY / WORKING / ...). Claude Code does not report a state, and
  guessing one from the transcript would be stale exactly when it matters -- the status line is
  not re-run when you submit a prompt.
- **No artifact / subagent / background-task counters or sandbox segment.** The payload has no
  such fields, and the footer and agent panel already show background work.
- **No Windows process inspection.** Claude Code exports `CLAUDE_PID`, so the launch flags are one
  read of `/proc/$CLAUDE_PID/cmdline` away.

Added in their place: cost, lines changed, session time, Claude Code's rate-limit windows, prompt
cache state, effort level and fast mode.

## How the permission mode is found

Claude Code re-runs the status line when the permission mode changes, but (as of v2.1.310) does
not include the mode in the payload. cc-statusline resolves it from, in order:

1. `permission_mode` in the payload, should a future release add it;
2. the newest `permissionMode` recorded in the session transcript (user prompts carry the mode they
   were sent under, and Claude Code re-appends a `permission-mode` metadata line periodically);
3. the Claude Code process's own flags: `--dangerously-skip-permissions` or
   `--permission-mode <mode>`;
4. `"yolo": true` in the config forces the YOLO badge regardless.

The transcript only learns about a shift+tab when the next turn writes it, so a mode change made
while idle reaches the badge after your next prompt. No tool runs in between, and Claude Code's own
footer below the status line always shows the live mode.

## Configuration

`~/.claude/cc-statusline/statusline_config.json` (or any file named by `CC_STATUSLINE_CONFIG`):

| Key | Effect |
| --- | --- |
| `use_nerd_fonts` | Nerd Font icons (the shipped config turns this on) |
| `use_ascii` | plain-text labels (`DIR:`, `CTX:`, ...) for terminals without icon fonts |
| `colors.foreground.*`, `colors.ui.separator` | `#RRGGBB` / `#RGB` colors and the segment separator |
| `icons.{ascii,nerd_fonts,emoji_fallback}.{components,mode,other}.*` | override any icon |
| `yolo` | always show the YOLO badge |

With neither font key set the line is ASCII. `USE_ASCII=true` or `USE_NERD_FONTS=false` in the
environment choose too, when the config does not. For an all-text setup:

```bash
cp ascii_statusline_config.json ~/.claude/cc-statusline/statusline_config.json
```

Claude Code indents the status row by two columns and truncates it with `…` two columns short of
the right edge, so lines are laid out to `COLUMNS - 4`. A `padding` value in your `statusLine`
settings eats into that further.

## Performance

A native binary that parses the payload once, reads `.git/HEAD` instead of spawning git, and scans
the transcript backwards only as far as its newest mode record. Measured over 200 runs against a
1.6 MB live transcript, process spawn included: **p50 3.4 ms, p95 5.3 ms**.

## Build and test

```bash
cargo build --release
cargo test
```

The unit tests cover each segment, the transcript scan and the layout. `tests/cli.rs` drives the
built binary with a controlled environment (`COLUMNS`, `HOME`, `CLAUDE_PID`, `CLAUDE_CONFIG_DIR`):
every line fits at widths from 24 to 300 columns, malformed input still renders, and a stand-in
process carrying `--dangerously-skip-permissions` lights the YOLO badge. Try it by hand:

```bash
COLUMNS=150 ./target/release/cc-statusline < tests/fixtures/full.json
```

## Credits & License

- Adapted from agy-statusline-linux / [agy-statusline-win](https://github.com/vmsysadm/agy-statusline-win),
  itself derived from [agy-statusline](https://codeberg.org/jochenkirstaetter/agy-statusline) by
  **Jochen Kirstätter**.
- Licensed under the **MIT License**.

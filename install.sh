#!/usr/bin/env bash
# Install cc-statusline as the Claude Code status line.
#
# usage: ./install.sh [--refresh SECONDS]
#   --refresh N  also re-run every N seconds (Claude Code's `refreshInterval`), so the
#                rate-limit countdowns and session timer stay current while the session is
#                idle. Default 30; 0 leaves it out and refreshes on Claude Code's events only.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLAUDE_DIR="${CLAUDE_CONFIG_DIR:-${HOME}/.claude}"
TARGET_DIR="${CLAUDE_DIR}/cc-statusline"
BIN_TARGET="${TARGET_DIR}/cc-statusline"
CONFIG_TARGET="${TARGET_DIR}/statusline_config.json"
PREVIOUS_PATH="${TARGET_DIR}/previous_statusline.json"
SETTINGS_PATH="${CLAUDE_DIR}/settings.json"
LOCAL_BIN="${SCRIPT_DIR}/target/release/cc-statusline"
REFRESH=30

while [ $# -gt 0 ]; do
    case "$1" in
        --refresh) REFRESH="${2:?--refresh needs a value}"; shift 2 ;;
        --refresh=*) REFRESH="${1#*=}"; shift ;;
        -h|--help) sed -n '2,7p' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done
case "${REFRESH}" in
    ''|*[!0-9]*) echo "--refresh must be a whole number of seconds" >&2; exit 2 ;;
esac

if command -v cargo >/dev/null 2>&1; then
    echo "Building cc-statusline (release)..."
    (cd "${SCRIPT_DIR}" && cargo build --release --quiet)
elif [ ! -x "${LOCAL_BIN}" ]; then
    echo "Error: cargo not found and no prebuilt binary at ${LOCAL_BIN}." >&2
    exit 1
fi

mkdir -p "${TARGET_DIR}"
# Copy to a temp name and rename over the target: Claude Code may be running the old binary at
# this moment, and overwriting an executing file in place fails with "Text file busy".
install -m 755 "${LOCAL_BIN}" "${BIN_TARGET}.new"
mv -f "${BIN_TARGET}.new" "${BIN_TARGET}"

if [ ! -f "${CONFIG_TARGET}" ]; then
    cp "${SCRIPT_DIR}/statusline_config.json" "${CONFIG_TARGET}"
    echo "Installed default config at ${CONFIG_TARGET}"
fi

python3 - "${SETTINGS_PATH}" "${BIN_TARGET}" "${REFRESH}" "${PREVIOUS_PATH}" <<'EOF'
import json, os, shlex, sys

settings_path, binary, refresh, previous_path = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
command = shlex.quote(binary)  # `command` runs in a shell

settings = {}
if os.path.exists(settings_path):
    with open(settings_path, encoding="utf-8") as f:
        try:
            settings = json.load(f)
        except json.JSONDecodeError as e:
            sys.exit(f"Refusing to modify {settings_path}: it is not valid JSON ({e}).")

# Keep the status line that was configured before, so uninstall can put it back. Re-running the
# installer over our own entry must not overwrite that record with ourselves.
current = settings.get("statusLine")
if current is not None and not (isinstance(current, dict) and current.get("command") == command):
    with open(previous_path, "w", encoding="utf-8") as f:
        json.dump(current, f, indent=2, ensure_ascii=False)
    print(f"Saved the previous statusLine to {previous_path}")

status_line = {"type": "command", "command": command, "padding": 0}
if refresh > 0:
    status_line["refreshInterval"] = refresh
settings["statusLine"] = status_line

tmp = settings_path + ".cc-statusline.tmp"
with open(tmp, "w", encoding="utf-8") as f:
    json.dump(settings, f, indent=2, ensure_ascii=False)
    f.write("\n")
os.replace(tmp, settings_path)
EOF

echo "cc-statusline is now the Claude Code status line (${BIN_TARGET})."
echo "Claude Code picks the change up live; no restart needed."

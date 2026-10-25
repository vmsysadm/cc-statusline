#!/usr/bin/env bash
# Remove cc-statusline from Claude Code, restoring whatever status line was configured before.
#
# usage: ./uninstall.sh [--purge]
#   --purge  also delete the config file (kept by default, for a later reinstall)
set -euo pipefail

CLAUDE_DIR="${CLAUDE_CONFIG_DIR:-${HOME}/.claude}"
TARGET_DIR="${CLAUDE_DIR}/cc-statusline"
BIN_TARGET="${TARGET_DIR}/cc-statusline"
CONFIG_TARGET="${TARGET_DIR}/statusline_config.json"
PREVIOUS_PATH="${TARGET_DIR}/previous_statusline.json"
SETTINGS_PATH="${CLAUDE_DIR}/settings.json"
PURGE=0

case "${1:-}" in
    --purge) PURGE=1 ;;
    -h|--help) sed -n '2,5p' "$0"; exit 0 ;;
    '') ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
esac

if [ -f "${SETTINGS_PATH}" ]; then
    python3 - "${SETTINGS_PATH}" "${BIN_TARGET}" "${PREVIOUS_PATH}" <<'EOF'
import json, os, shlex, sys

settings_path, binary, previous_path = sys.argv[1], sys.argv[2], sys.argv[3]
command = shlex.quote(binary)

with open(settings_path, encoding="utf-8") as f:
    try:
        settings = json.load(f)
    except json.JSONDecodeError as e:
        sys.exit(f"Refusing to modify {settings_path}: it is not valid JSON ({e}).")

current = settings.get("statusLine")
if not (isinstance(current, dict) and current.get("command") == command):
    print("statusLine does not point at cc-statusline; leaving settings.json unchanged.")
    sys.exit(0)

if os.path.exists(previous_path):
    with open(previous_path, encoding="utf-8") as f:
        settings["statusLine"] = json.load(f)
    print("Restored the previous statusLine.")
else:
    del settings["statusLine"]
    print("Removed statusLine from settings.json.")

tmp = settings_path + ".cc-statusline.tmp"
with open(tmp, "w", encoding="utf-8") as f:
    json.dump(settings, f, indent=2, ensure_ascii=False)
    f.write("\n")
os.replace(tmp, settings_path)
EOF
fi

rm -f "${BIN_TARGET}" "${PREVIOUS_PATH}"
if [ "${PURGE}" = 1 ]; then
    rm -f "${CONFIG_TARGET}"
    rmdir "${TARGET_DIR}" 2>/dev/null || true
    echo "Removed ${TARGET_DIR}."
else
    echo "Removed the binary; kept ${CONFIG_TARGET} (use --purge to delete it)."
fi

#!/usr/bin/env bash
# Format a single file after an agent edits it.
#
# Wired into .claude/settings.json as a PostToolUse hook for Edit|Write. Claude Code passes the
# tool payload as JSON on stdin; we format only the file that was touched, not the whole tree.
#
# This hook must never fail a tool call: a missing formatter is a setup problem, not an edit
# problem. Every path exits 0.
set -uo pipefail

command -v python3 >/dev/null 2>&1 || exit 0

file="$(python3 -c '
import json, sys
try:
    print(json.load(sys.stdin).get("tool_input", {}).get("file_path", ""))
except Exception:
    print("")
' 2>/dev/null)" || exit 0

[[ -n "$file" && -f "$file" ]] || exit 0

# Only format files inside this repo; the agent may edit scratchpad or plan files elsewhere.
repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
[[ "$(realpath "$file")" == "$repo_root"/* ]] || exit 0

case "$file" in
    *.dart)
        command -v dart >/dev/null 2>&1 && dart format --line-length 100 "$file" >/dev/null 2>&1
        ;;
    *.rs)
        command -v rustfmt >/dev/null 2>&1 && rustfmt --edition 2024 "$file" >/dev/null 2>&1
        ;;
esac

exit 0

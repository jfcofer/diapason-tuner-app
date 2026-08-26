#!/usr/bin/env bash
# Verify the session protocol in AGENTS.md §2 was actually followed before the session closes.
#
# The context system only works if STATE.md, the task file and the journal are current. Every one
# of them is easy to forget and expensive for the next agent to be missing, so this checks rather
# than trusts.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; OFF=''
fi

FAILURES=0
fail() { printf '  %s✗%s %s\n' "$RED" "$OFF" "$1"; [[ -n "${2:-}" ]] && printf '      %s%s%s\n' "$DIM" "$2" "$OFF"; FAILURES=$((FAILURES+1)); }
ok()   { printf '  %s✓%s %s\n' "$GREEN" "$OFF" "$1"; }

printf '\nSession protocol (AGENTS.md §2)\n'

# Compare against the last commit: what changed in this working session?
changed="$(git status --porcelain; git diff --name-only HEAD~1 2>/dev/null)"

# 1. STATE.md must have been touched if any code or docs changed.
if grep -qE '^(M|A|\?\?)? *(apps|packages|rust|docs|tools)/' <<<"$changed" 2>/dev/null; then
    if grep -q "docs/agents/STATE.md" <<<"$changed"; then
        ok "STATE.md updated"
    else
        fail "STATE.md was not updated" \
             "It is a snapshot of the current truth, not a log. Rewrite it before ending."
    fi
else
    ok "nothing changed, STATE.md update not required"
fi

# 2. A journal entry for today.
today="$(date +%Y-%m-%d)"
if compgen -G "docs/agents/journal/${today}-*.md" >/dev/null; then
    ok "journal entry exists for $today"
else
    fail "no journal entry for $today" \
         "Append one: docs/agents/journal/${today}-<slug>.md (template in that folder)."
fi

# 3. STATE.md must name a task file that exists.
active="$(grep -oP 'docs/agents/tasks/\KT-[0-9a-z-]+\.md' docs/agents/STATE.md 2>/dev/null | head -1)"
if [[ -n "$active" && -f "docs/agents/tasks/$active" ]]; then
    ok "active task exists: $active"
elif [[ -n "$active" ]]; then
    fail "STATE.md names a task file that does not exist: $active"
else
    fail "STATE.md does not name an active task file"
fi

# 4. STATE.md must be a snapshot, not a log.
if [[ -f docs/agents/STATE.md ]]; then
    lines=$(wc -l < docs/agents/STATE.md)
    if [[ $lines -gt 120 ]]; then
        fail "STATE.md is $lines lines (budget 120)" "It is a snapshot. Delete what is no longer true."
    else
        ok "STATE.md is $lines/120 lines"
    fi
fi

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ the next agent will be missing something%s\n\n' "$RED" "$OFF"; exit 1
fi
printf '%s✓ session protocol followed%s\n\n' "$GREEN" "$OFF"

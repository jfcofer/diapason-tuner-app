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

# What this session changed, measured from where it started: `session-start` records HEAD in the
# git dir. Without that mark (session-start not run), fall back to where the branch left
# origin/main, which on a multi-session branch also counts earlier sessions' work, and say so.
# `git diff <base>` covers committed, staged and unstaged in one call; `-z` paths are never quoted,
# so spaces and non-ASCII names count too (T-010).
mark="$(git rev-parse --git-path diapason-session-base)"
base=''
if [[ -f "$mark" ]] && git merge-base --is-ancestor "$(cat "$mark")" HEAD 2>/dev/null; then
    base="$(cat "$mark")"
elif base="$(git merge-base HEAD origin/main 2>/dev/null)"; then
    printf '  %sno session-start mark: measuring from origin/main%s\n' "$DIM" "$OFF"
else
    base="$(git rev-parse HEAD)"
    printf '  %sno session-start mark and no origin/main: measuring uncommitted work only%s\n' "$DIM" "$OFF"
fi
# The rules a PR is also held to (STATE updated, code recorded in a task file, the journal
# append-only) live in pr-check.sh, run here on the session's diff plus uncommitted work (T-019).
"$(dirname "${BASH_SOURCE[0]}")/pr-check.sh" --session "$base" || FAILURES=$((FAILURES+1))

printf 'Session only\n'
work="$( { git diff --name-only -z "$base"; git ls-files -o --exclude-standard -z; } | tr '\0' '\n' | grep -v '^docs/agents/')"

# 1. A rewritten STATE.md is dated today: an old header tells the next agent the snapshot is older
# than it is.
today="$(date +%Y-%m-%d)"
if [[ -z "$work" ]]; then
    ok "nothing changed, STATE.md date not checked"
elif grep -qE "^# STATE — updated $today\b" docs/agents/STATE.md; then
    ok "STATE.md dated $today"
else
    fail "STATE.md's header is not dated $today" \
         "Rewrite the first line: # STATE — updated $today (<task and part>)."
fi

# 2. A journal entry for today.
if compgen -G "docs/agents/journal/${today}-*.md" >/dev/null; then
    ok "journal entry exists for $today"
else
    fail "no journal entry for $today" \
         "Append one: docs/agents/journal/${today}-<slug>.md (template in that folder)."
fi

# 3. STATE.md must name a task file that exists. Any task counts: a session that closes one points
# STATE at the next.
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

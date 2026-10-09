#!/usr/bin/env bash
# Show where the repository actually is before an agent trusts STATE.md (AGENTS.md §2).
#
# STATE.md is written at the end of a session, but merges, Dependabot and CI all happen between
# sessions. This prints that out-of-band state so the agent can reconcile STATE.md against it
# instead of acting on a snapshot that went stale the moment the owner merged something (T-007).

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ -t 1 ]]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'; OFF=$'\033[0m'
else
    BOLD=''; DIM=''; OFF=''
fi

section() { printf '\n%s%s%s\n' "$BOLD" "$1" "$OFF"; }
note()    { printf '  %s%s%s\n' "$DIM" "$1" "$OFF"; }

section "Read, in order"
printf '  1. AGENTS.md\n  2. docs/agents/STATE.md\n  3. the active task file named in STATE.md\n'
printf '  4. only the reference docs that task points to\n'
note "Then reconcile STATE.md with everything below. Report any contradiction first."

section "Working tree"
git -c color.ui=always log --oneline -8
status="$(git status --short --branch)"
printf '%s\n' "$status"

section "Branches not merged into main"
git fetch --quiet --prune origin 2>/dev/null || note "git fetch failed; remote branches may be stale"
unmerged="$(git branch --all --no-merged origin/main --format='%(refname:short)' 2>/dev/null)"
if [[ -n "$unmerged" ]]; then
    while read -r branch; do printf '  %s\n' "$branch"; done <<<"$unmerged"
else
    note "none"
fi

section "Open pull requests"
# One verdict per PR from its checks: any failure is red; anything unfinished or unknown is pending.
verdict_jq="$(cat <<'JQ'
.[] | [.number,
    ([.statusCheckRollup[] | (.conclusion // "") | ascii_upcase] as $c
     | if ($c | any(. == "FAILURE" or . == "CANCELLED" or . == "TIMED_OUT" or . == "ACTION_REQUIRED"))
         then "checks failing"
       elif ($c | length) > 0 and ($c | all(. == "SUCCESS" or . == "SKIPPED" or . == "NEUTRAL"))
         then "checks green"
       else "checks pending" end),
    .headRefName, .title] | @tsv
JQ
)"
if ! command -v gh >/dev/null 2>&1; then
    note "gh is not installed; check open PRs and their checks on GitHub by hand"
elif ! prs="$(gh pr list --state open --limit 30 --json number,title,headRefName,statusCheckRollup \
        --jq "$verdict_jq" 2>/dev/null)"; then
    note "gh could not list PRs (offline or not authenticated); check GitHub by hand"
elif [[ -z "$prs" ]]; then
    note "none"
else
    while IFS=$'\t' read -r number verdict branch title; do
        printf '  #%-4s %-15s %s  %s(%s)%s\n' "$number" "$verdict" "$title" "$DIM" "$branch" "$OFF"
    done <<<"$prs"
fi

section "Toolchain"
just doctor

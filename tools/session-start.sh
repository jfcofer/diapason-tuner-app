#!/usr/bin/env bash
# Show where the repository actually is before an agent trusts STATE.md (AGENTS.md §2).
#
# STATE.md is written at the end of a session, but merges, Dependabot and CI all happen between
# sessions. This prints that out-of-band state so the agent can reconcile STATE.md against it
# instead of acting on a snapshot that went stale the moment the owner merged something (T-007).
#
# It makes two read-only network calls, `git fetch` and `gh pr list`, which the owner approved for
# this recipe (CLAUDE.md, Permissions). Both are bounded and fall back to a notice, never a failure.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ -t 1 ]]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'; OFF=$'\033[0m'
else
    BOLD=''; DIM=''; OFF=''
fi

section() { printf '\n%s%s%s\n' "$BOLD" "$1" "$OFF"; }
note()    { printf '  %s%s%s\n' "$DIM" "$1" "$OFF"; }

# Run a network command without letting it hang the session: no credential or passphrase prompt,
# a short SSH connect timeout, and a hard limit where `timeout` exists (it is not on stock macOS).
bounded() {
    local deadline=()
    command -v timeout >/dev/null 2>&1 && deadline=(timeout 20)
    # The `+` form keeps an empty array from tripping `set -u` on bash 3.2.
    GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND="ssh -o BatchMode=yes -o ConnectTimeout=5" \
        GH_PROMPT_DISABLED=1 ${deadline[@]+"${deadline[@]}"} "$@"
}

errors="$(mktemp)"
trap 'rm -f "$errors"' EXIT
first_error() { head -1 "$errors"; }

section "Read, in order"
printf '  1. AGENTS.md\n  2. docs/agents/STATE.md\n  3. the active task file named in STATE.md\n'
printf '  4. only the reference docs that task points to\n'
note "Then reconcile STATE.md with everything below. Report any contradiction first."

section "Working tree"
git -c color.ui=auto log --oneline -8
git status --short --branch

section "Branches"
bounded git fetch --quiet --prune origin 2>"$errors" \
    || note "git fetch failed, so remote branches may be stale: $(first_error)"
if ! git rev-parse --verify --quiet origin/main >/dev/null; then
    note "no origin/main, so merged and unmerged branches cannot be told apart"
else
    # main only accepts rebase-merges, which rewrite every SHA, so ancestry (`--no-merged`) calls a
    # merged branch unmerged forever. `git cherry` compares patches instead: a branch with no
    # commit missing from main is merged, whatever its SHAs.
    while read -r ref track; do
        [[ "$ref" == main || "$ref" == origin || "$ref" == origin/HEAD || "$ref" == origin/main ]] && continue
        pending="$(git cherry origin/main "$ref" 2>/dev/null | grep -c '^+')"
        if [[ "$pending" == 0 ]]; then
            state="merged into main"
        else
            state="$pending commit(s) not in main"
        fi
        [[ "$track" == *gone* ]] && state="$state, upstream deleted"
        printf '  %-48s %s\n' "$ref" "$state"
    done < <(git for-each-ref --format='%(refname:short) %(upstream:track)' refs/heads refs/remotes/origin)
fi

section "Pull requests"
# One verdict per PR. Checks report `conclusion`; legacy commit statuses report `state`. Any
# failure is red, all-green needs every entry to have passed, and anything else is pending.
verdict_jq="$(cat <<'JQ'
.[] | [.number,
    ([(.statusCheckRollup // [])[] | (.conclusion // .state // "") | ascii_upcase] as $c
     | if ($c | any(. == "FAILURE" or . == "ERROR" or . == "CANCELLED" or . == "TIMED_OUT"
                    or . == "ACTION_REQUIRED" or . == "STARTUP_FAILURE"))
         then "checks failing"
       elif ($c | length) > 0 and ($c | all(. == "SUCCESS" or . == "SKIPPED" or . == "NEUTRAL"))
         then "checks green"
       else "checks pending" end),
    .headRefName, .title] | @tsv
JQ
)"
pr_limit=30
if ! command -v gh >/dev/null 2>&1; then
    note "gh is not installed; check PRs and their checks on GitHub by hand"
elif ! open_prs="$(bounded gh pr list --state open --limit "$pr_limit" \
        --json number,title,headRefName,statusCheckRollup --jq "$verdict_jq" 2>"$errors")"; then
    note "gh could not list PRs; check GitHub by hand: $(first_error)"
else
    printf '  %sOpen%s\n' "$BOLD" "$OFF"
    if [[ -z "$open_prs" ]]; then
        note "none"
    else
        while IFS=$'\t' read -r number verdict branch title; do
            printf '  #%-4s %-15s %s  %s(%s)%s\n' "$number" "$verdict" "$title" "$DIM" "$branch" "$OFF"
        done <<<"$open_prs"
        [[ "$(wc -l <<<"$open_prs")" -ge "$pr_limit" ]] && note "only the first $pr_limit are shown"
    fi
    printf '  %sRecently merged%s\n' "$BOLD" "$OFF"
    merged="$(bounded gh pr list --state merged --limit 5 --json number,title,headRefName,mergedAt \
        --jq '.[] | [.number, .mergedAt[:10], .headRefName, .title] | @tsv' 2>"$errors")" \
        || note "gh could not list merged PRs: $(first_error)"
    while IFS=$'\t' read -r number date branch title; do
        [[ -n "$number" ]] && printf '  #%-4s %-15s %s  %s(%s)%s\n' "$number" "$date" "$title" "$DIM" "$branch" "$OFF"
    done <<<"$merged"
fi

section "Toolchain"
just doctor

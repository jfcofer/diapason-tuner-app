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
# Mark where this session starts, so session-end measures this session's work and not the branch's
# (T-010). Running this again mid-session moves the mark forward.
git rev-parse HEAD >"$(git rev-parse --git-path diapason-session-base)"
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
open_prs='' merged='' have_prs=0
if ! command -v gh >/dev/null 2>&1; then
    note "gh is not installed; check PRs and their checks on GitHub by hand"
elif ! open_prs="$(bounded gh pr list --state open --limit "$pr_limit" \
        --json number,title,headRefName,statusCheckRollup --jq "$verdict_jq" 2>"$errors")"; then
    note "gh could not list PRs; check GitHub by hand: $(first_error)"
else
    have_prs=1
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
    # Newest first. More are fetched than shown, because "Open tasks" below looks further back.
    merged="$(bounded gh pr list --state merged --limit "$pr_limit" \
        --json number,title,headRefName,mergedAt \
        --jq 'sort_by(.mergedAt) | reverse | .[] | [.number, .mergedAt[:10], .headRefName, .title] | @tsv' \
        2>"$errors")" || { note "gh could not list merged PRs: $(first_error)"; have_prs=0; }
    while IFS=$'\t' read -r number date branch title; do
        [[ -n "$number" ]] && printf '  #%-4s %-15s %s  %s(%s)%s\n' "$number" "$date" "$title" "$DIM" "$branch" "$OFF"
    done < <(head -5 <<<"$merged")
fi

section "Open tasks"
# A task is closed by hand, and the merge that should prompt it happens on GitHub between sessions,
# usually deleting its branch on the way. Matching each open task against PRs (branch names carry
# the ID: AGENTS.md §7) is what catches a task left in-progress after its work merged (T-009).
# Multi-part tasks merge a PR per part, so a merged PR is a prompt to check, not proof of done.
note "Each open task against its PRs. Reconcile the task file with any mismatch."
for task in docs/agents/tasks/T-*.md; do
    id="$(sed -n '2,/^---$/s/^id: *//p' "$task")"
    status="$(sed -n '2,/^---$/s/^status: *//p' "$task")"
    [[ "$status" == todo || "$status" == in-progress || "$status" == blocked ]] || continue
    # A parent never has a PR of its own: its slices carry the work, so show theirs (T-010).
    slices=''
    for child in docs/agents/tasks/"$id"[a-z]-*.md; do
        [[ -f "$child" ]] || continue
        slices+="$(sed -n '2,/^---$/s/^id: *//p' "$child") $(sed -n '2,/^---$/s/^status: *//p' "$child"), "
    done
    if [[ -n "$slices" ]]; then
        printf '  %-8s %-12s %s\n' "$id" "$status" "parent; slices: ${slices%, }"
        continue
    fi
    if [[ $have_prs -eq 0 ]]; then
        [[ "$status" == todo ]] && continue
        printf '  %-8s %-12s %s\n' "$id" "$status" "PR state unknown (see above)"
        continue
    fi
    # Field 3 is the branch in both lists. "/T-002-" must not match "feat/T-002b-…".
    open_pr="$(awk -F'\t' -v id="$id" 'index($3, "/" id "-") { print "#" $1; exit }' <<<"$open_prs")"
    last_merged="$(awk -F'\t' -v id="$id" \
        'index($3, "/" id "-") { print "#" $1 " merged " $2; exit }' <<<"$merged")"
    # A todo task is listed only when a PR already carries its ID: then it is not todo (T-010).
    if [[ "$status" == todo ]]; then
        [[ -n "$open_pr$last_merged" ]] || continue
        verdict="has PR ${open_pr:-$last_merged}, so it has started. Mark it in-progress or done"
    elif [[ -n "$open_pr" ]]; then
        verdict="PR $open_pr open"
    elif [[ -n "$last_merged" ]]; then
        verdict="no open PR; last PR $last_merged. Close the task, or confirm it says what is left"
    else
        verdict="no open PR, and none among the last $pr_limit merged"
    fi
    printf '  %-8s %-12s %s\n' "$id" "$status" "$verdict"
done

section "Toolchain"
just doctor

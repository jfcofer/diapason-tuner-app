#!/usr/bin/env bash
# Check the session protocol (AGENTS.md §2) on a branch's diff, so it binds every agent and human
# who opens a PR, not only one that remembers `just session-end` (T-019).
#
#   tools/pr-check.sh BASE [BRANCH]        PR mode: what CI runs on every pull request
#   tools/pr-check.sh --session BASE       session mode: `session-end` runs the shared rules
#   tools/pr-check.sh --self-test          prove each rule fails on a planted branch
#
# BASE is a revision; the diff is measured from where the branch left it (`BASE...HEAD`). BRANCH
# defaults to the current branch; CI passes the PR's head branch, since it checks out a merge
# commit. `-C DIR` runs against another repository (the self-test uses it).
#
# Rules, PR mode:
#   1. anything outside docs/agents/ changed  ⇒  docs/agents/STATE.md changed
#   2. anything outside docs/ changed         ⇒  the branch's task file changed
#   3. at least one journal entry added, and no existing entry modified or deleted
#   4. the branch's task is done, blocked or abandoned, or in-progress with a `**Next part:**`
#      line: the PR states what its task is after the merge, because nothing can after it
# The branch must be named `<type>/T-###x-slug` (AGENTS.md §7); `dependabot/*` is exempt.
# Session mode applies 1, 2 (any task file) and the journal half of 3 that forbids edits, to the
# diff plus uncommitted and untracked files; `session-end` adds the session-only checks.

set -uo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; OFF=''
fi

FAILURES=0
fail() { printf '  %s✗%s %s\n' "$RED" "$OFF" "$1"; [[ -n "${2:-}" ]] && printf '      %s%s%s\n' "$DIM" "$2" "$OFF"; FAILURES=$((FAILURES+1)); }
ok()   { printf '  %s✓%s %s\n' "$GREEN" "$OFF" "$1"; }
note() { printf '  %s%s%s\n' "$DIM" "$1" "$OFF"; }

# --- self-test ------------------------------------------------------------------------------------
# Each case builds a branch in a throwaway repository and asserts the exit status. A rule that
# stops failing when it should is caught here, inside `just verify`.
self_test() {
    local tmp; tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN
    local -a g=(git -C "$tmp" -c user.name=pr-check -c user.email=pr-check@localhost -c commit.gpgsign=false)
    "${g[@]}" init -q -b main
    mkdir -p "$tmp/docs/agents/journal" "$tmp/docs/agents/tasks" "$tmp/src"
    printf '# STATE\n' > "$tmp/docs/agents/STATE.md"
    printf '# template\n' > "$tmp/docs/agents/journal/README.md"
    printf 'old\n' > "$tmp/docs/agents/journal/2026-01-01-old.md"
    printf -- '---\nid: T-900\nstatus: todo\n---\n' > "$tmp/docs/agents/tasks/T-900-case.md"
    printf 'fn a() {}\n' > "$tmp/src/a.rs"
    "${g[@]}" add -A && "${g[@]}" commit -qm base

    local passed=0 failed=0
    # case NAME EXPECT(pass|fail) BRANCH EDITS…  where each edit is one of:
    #   code  state  journal  old-journal  task:<status>  next-part
    case_() {
        local name="$1" expect="$2" branch="$3"; shift 3
        "${g[@]}" checkout -q main && "${g[@]}" checkout -q -b "$branch"
        local edit
        for edit in "$@"; do
            case "$edit" in
                code)        printf '// %s\n' "$name" >> "$tmp/src/a.rs" ;;
                state)       printf '%s\n' "$name" >> "$tmp/docs/agents/STATE.md" ;;
                journal)     printf 'entry\n' > "$tmp/docs/agents/journal/2026-02-02-${branch//\//-}.md" ;;
                old-journal) printf 'rewritten\n' >> "$tmp/docs/agents/journal/2026-01-01-old.md" ;;
                task:*)      sed -i "s/^status: .*/status: ${edit#task:}/" "$tmp/docs/agents/tasks/T-900-case.md" ;;
                next-part)   printf '**Next part:** the rest\n' >> "$tmp/docs/agents/tasks/T-900-case.md" ;;
            esac
        done
        "${g[@]}" add -A && "${g[@]}" commit -qm "$name" --allow-empty
        local got=pass
        "$SCRIPT" -C "$tmp" main "$branch" >/dev/null 2>&1 || got=fail
        if [[ "$got" == "$expect" ]]; then
            ok "$name: $( [[ $expect == pass ]] && echo passes || echo fails )"; passed=$((passed+1))
        else
            fail "$name: expected $expect, got $got"; failed=$((failed+1))
        fi
    }

    printf '\npr-check self-test\n'
    case_ "complete task branch"            pass feat/T-900-a code state journal task:done
    case_ "multi-part task, next part named" pass feat/T-900-b code state journal task:in-progress next-part
    case_ "blocked task"                    pass feat/T-900-c code state journal task:blocked
    case_ "docs-only branch, no task edit"  pass docs/T-900-d state journal task:done
    case_ "dependabot, code only"           pass dependabot/cargo/x code
    case_ "code without a task edit"        fail feat/T-900-e code state journal
    case_ "no STATE edit"                   fail feat/T-900-f code journal task:done
    case_ "no journal entry"                fail feat/T-900-g code state task:done
    case_ "existing journal entry edited"   fail feat/T-900-h code state journal old-journal task:done
    case_ "in-progress, no next part"       fail feat/T-900-i code state journal task:in-progress
    case_ "task left todo"                  fail feat/T-900-j code state journal task:todo
    case_ "branch names no task"            fail feat/no-task code state journal task:done
    case_ "branch names a missing task"     fail feat/T-901-k code state journal task:done

    printf '\n'
    if [[ $failed -gt 0 ]]; then
        printf '%s✗ pr-check self-test: %d of %d failed%s\n\n' "$RED" "$failed" "$((passed+failed))" "$OFF"
        return 1
    fi
    printf '%s✓ pr-check self-test: %d cases%s\n\n' "$GREEN" "$passed" "$OFF"
}

# --- arguments ------------------------------------------------------------------------------------
mode="pr"
repo="$(dirname "$SCRIPT")/.."
args=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --self-test) self_test; exit $? ;;
        --session)   mode=session; shift ;;
        -C)          repo="$2"; shift 2 ;;
        *)           args+=("$1"); shift ;;
    esac
done
cd "$repo" || exit 1
base_rev="${args[0]:-origin/main}"
branch="${args[1]:-$(git rev-parse --abbrev-ref HEAD)}"

if ! base="$(git merge-base "$base_rev" HEAD 2>/dev/null)"; then
    printf '  %s✗%s no merge base with %s (in CI, check out with fetch-depth: 0)\n' "$RED" "$OFF" "$base_rev"
    exit 1
fi

printf '\nSession protocol on the diff from %s (%s)\n' "$base_rev" "$branch"

if [[ "$mode" == pr && "$branch" == dependabot/* ]]; then
    note "dependabot branch: no task, STATE or journal expected"
    printf '\n%s✓ protocol followed%s\n\n' "$GREEN" "$OFF"; exit 0
fi

# Committed changes in PR mode; in session mode also staged, unstaged and untracked. `-z` paths are
# never quoted, so spaces and non-ASCII names count too (T-010).
if [[ "$mode" == pr ]]; then
    changed="$(git diff --name-only -z "$base" HEAD | tr '\0' '\n' | sort -u)"
    status="$(git diff --name-status "$base" HEAD)"
else
    changed="$( { git diff --name-only -z "$base"; git ls-files -o --exclude-standard -z; } \
        | tr '\0' '\n' | sort -u)"
    status="$(git diff --name-status "$base")"
fi
touched() { grep -qxF "$1" <<<"$changed"; }
work="$(grep -v '^docs/agents/' <<<"$changed")"
code="$(grep -v '^docs/' <<<"$work")"

# The branch's task, from its name (AGENTS.md §7: feat/T-012-strobe-ring).
task_id="$(grep -oP '^[a-z]+/\KT-[0-9]{3}[a-z]?(?=-)' <<<"$branch")"
task_file=''
if [[ -n "$task_id" ]]; then
    task_file="$(compgen -G "docs/agents/tasks/${task_id}-*.md" | head -1)"
fi
if [[ "$mode" == pr ]]; then
    if [[ -z "$task_id" ]]; then
        fail "branch \`$branch\` names no task" \
             "Name it <type>/T-###-slug (AGENTS.md §7). Work without a task starts with one (§3.1)."
    elif [[ -z "$task_file" ]]; then
        fail "branch names $task_id, which has no file in docs/agents/tasks/"
    fi
fi

# 1. STATE.md
if [[ -z "$work" ]]; then
    ok "only session files changed, STATE.md update not required"
elif touched docs/agents/STATE.md; then
    ok "STATE.md updated"
else
    fail "STATE.md was not updated" "Rewrite the snapshot as it will be after this merges."
fi

# 2. The task file
if [[ -z "$code" ]]; then
    ok "no code changed, task file update not required"
elif [[ "$mode" == pr && -n "$task_file" ]]; then
    if touched "$task_file"; then
        ok "$task_file updated"
    else
        fail "code changed but $task_file did not" \
             "Tick the criteria met, and record deviations in its Implementation notes."
    fi
elif grep -qE '^docs/agents/tasks/T-[^/]+\.md$' <<<"$changed"; then
    ok "a task file was updated"
elif [[ "$mode" == session ]]; then
    fail "code changed but no task file was updated" \
         "Tick the criteria met, and record deviations in the task's Implementation notes."
fi

# 3. The journal is append-only (AGENTS.md §8), and a PR adds to it.
rewritten="$(awk '$1 ~ /^[MDR]/ && $2 ~ /^docs\/agents\/journal\/[0-9].*\.md$/ { print $2 }' <<<"$status")"
if [[ -n "$rewritten" ]]; then
    fail "existing journal entries were modified or deleted: $(tr '\n' ' ' <<<"$rewritten")" \
         "The journal is append-only. Add a new entry instead."
else
    ok "no existing journal entry modified"
fi
if [[ "$mode" == pr ]]; then
    added="$(awk '$1 == "A" && $2 ~ /^docs\/agents\/journal\/[0-9].*\.md$/ { print $2 }' <<<"$status")"
    if [[ -n "$added" ]]; then
        ok "journal: $(wc -l <<<"$added") entr$( [[ $(wc -l <<<"$added") -eq 1 ]] && echo y || echo ies) added"
    else
        fail "no journal entry added" "Append one: docs/agents/journal/YYYY-MM-DD-<slug>.md."
    fi
fi

# 4. The task's status after the merge.
if [[ "$mode" == pr && -n "$task_file" ]]; then
    task_status="$(grep -m1 -oP '^status:\s*\K\S+' "$task_file")"
    case "$task_status" in
        done|blocked|abandoned)
            ok "$task_id is $task_status" ;;
        in-progress)
            if grep -q '^\*\*Next part:\*\*' "$task_file"; then
                ok "$task_id stays in-progress, and names its next part"
            else
                fail "$task_id is in-progress with no \`**Next part:**\` line" \
                     "Mark it done if this PR finishes it. If not, say what the next PR does."
            fi ;;
        *)
            fail "$task_id is ${task_status:-without a status} on its own PR" \
                 "A task with a PR is at least in-progress." ;;
    esac
fi

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ %d protocol failure(s): the next agent will be missing something%s\n\n' "$RED" "$FAILURES" "$OFF"
    exit 1
fi
printf '%s✓ protocol followed%s\n\n' "$GREEN" "$OFF"

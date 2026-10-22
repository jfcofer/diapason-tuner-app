#!/usr/bin/env bash
# Link check, line budgets, and ADR index consistency.
#
# Documentation rots silently: a link breaks, a budget is quietly exceeded, an ADR is written and
# never indexed. None of that fails a build unless something checks it, so this runs in
# `just verify` alongside the tests.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; BOLD=$'\033[1m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; BOLD=''; OFF=''
fi

FAILURES=0
fail() { printf '  %s✗%s %s\n' "$RED" "$OFF" "$1"; [[ -n "${2:-}" ]] && printf '      %s%s%s\n' "$DIM" "$2" "$OFF"; FAILURES=$((FAILURES+1)); }
ok()   { printf '  %s✓%s %s\n' "$GREEN" "$OFF" "$1"; }

# ── Line budgets (AGENTS.md, adr/README.md, tasks/README.md state these) ──────
printf '\n%sLine budgets%s\n' "$BOLD" "$OFF"
check_budget() {
    local file="$1" budget="$2"
    [[ -f "$file" ]] || return 0
    local lines; lines=$(wc -l < "$file")
    if [[ $lines -gt $budget ]]; then
        fail "$file: $lines lines (budget $budget)" \
             "Budgets exist because these files are loaded into every agent's context."
    else
        ok "$file: $lines/$budget"
    fi
}
check_budget AGENTS.md 200
check_budget docs/agents/STATE.md 120
for adr in docs/adr/[0-9]*.md; do check_budget "$adr" 120; done
for task in docs/agents/tasks/T-*.md; do check_budget "$task" 150; done

# ── ADR index consistency ────────────────────────────────────────────────────
printf '\n%sADR index%s\n' "$BOLD" "$OFF"
missing=0
for adr in docs/adr/[0-9]*.md; do
    base="$(basename "$adr")"
    if ! grep -q "($base)" docs/adr/README.md; then
        fail "$base is not in the ADR index" "Add a row to docs/adr/README.md."
        missing=1
    fi
done
# And the reverse: an index row pointing at a file that does not exist.
while read -r linked; do
    [[ -f "docs/adr/$linked" ]] || { fail "ADR index links missing file: $linked"; missing=1; }
done < <(grep -oP '\]\(\K[0-9]{4}-[^)]+\.md' docs/adr/README.md 2>/dev/null)
[[ $missing -eq 0 ]] && ok "every ADR is indexed, and every indexed ADR exists"

# Status lines must be one of the documented values.
badstatus=0
for adr in docs/adr/[0-9]*.md; do
    grep -qE '^\*\*Status:\*\* (Accepted|Proposed|Rejected|Superseded by [0-9]{4})' "$adr" || {
        fail "$(basename "$adr"): missing or malformed Status line"; badstatus=1; }
done
[[ $badstatus -eq 0 ]] && ok "every ADR has a valid Status line"

# ── Relative links resolve ───────────────────────────────────────────────────
printf '\n%sRelative links%s\n' "$BOLD" "$OFF"
broken=0
while IFS=: read -r file link; do
    [[ -z "$link" ]] && continue
    target="$(dirname "$file")/${link%%#*}"
    [[ -e "$target" ]] || { fail "$file -> $link"; broken=$((broken+1)); }
done < <(
    find . -name '*.md' -not -path './.git/*' -not -path '*/build/*' -not -path '*/.dart_tool/*' \
        -not -path './target/*' -not -path '*/cargokit/*' -print0 |
    xargs -0 grep -oPH '\]\(\K(?!https?:|mailto:|#)[^)]+' 2>/dev/null
)
[[ $broken -eq 0 ]] && ok "all relative links resolve"

# ── Task files: front matter, and what "done" may leave open ─────────────────
# tasks/README.md defines the statuses. A done task may keep an unticked criterion only when the
# line says why it was closed; otherwise "done" means less than it claims.
printf '\n%sTask files%s\n' "$BOLD" "$OFF"
taskbad=0
front_matter() { awk 'NR==1 && $0=="---" {f=1; next} f && $0=="---" {exit} f' "$1"; }
for task in docs/agents/tasks/T-*.md; do
    base="$(basename "$task")"
    fm="$(front_matter "$task")"
    id="$(sed -n 's/^id: *//p' <<<"$fm")"
    status="$(sed -n 's/^status: *//p' <<<"$fm")"
    [[ "$base" == "${id}-"* ]] || { fail "$base: front matter id '${id}' does not match the file name"; taskbad=1; }
    case "$status" in
        todo|in-progress|blocked|done|abandoned) ;;
        *) fail "$base: status '${status}' is not one of todo|in-progress|blocked|done|abandoned"; taskbad=1 ;;
    esac
    if [[ "$status" == "done" ]]; then
        # A criterion wraps onto indented lines; judge the whole item, report its first line.
        while read -r open; do
            fail "$base is done but leaves a criterion open without saying it was closed" "$open"
            taskbad=1
        done < <(awk '
            function flush() { if (item != "" && tolower(item) !~ /closed/) print head; item = "" }
            /^- \[ \]/      { flush(); head = $0; item = $0; next }
            item != "" && /^  +[^ ]/ { item = item " " $0; next }
                            { flush() }
            END             { flush() }' "$task")
    fi
done
# A parent (T-002) is split into slices (T-002a, T-002b). A parent still todo once a slice has
# started, or done while one is open, says something false about the work (T-010).
for task in docs/agents/tasks/T-[0-9][0-9][0-9]-*.md; do
    parent="$(front_matter "$task" | sed -n 's/^id: *//p')"
    parent_status="$(front_matter "$task" | sed -n 's/^status: *//p')"
    for child in docs/agents/tasks/"$parent"[a-z]-*.md; do
        [[ -f "$child" ]] || continue
        child_status="$(front_matter "$child" | sed -n 's/^status: *//p')"
        if [[ "$parent_status" == todo && "$child_status" != todo ]]; then
            fail "$(basename "$task") is todo, but its slice $(basename "$child") is $child_status" \
                 "Mark the parent in-progress."
            taskbad=1
        elif [[ "$parent_status" == "done" && "$child_status" != "done" && "$child_status" != "abandoned" ]]; then
            fail "$(basename "$task") is done, but its slice $(basename "$child") is $child_status" \
                 "Finish or abandon the slice, or reopen the parent."
            taskbad=1
        fi
    done
done
active="$(grep -oP 'docs/agents/tasks/\KT-[0-9a-z-]+\.md' docs/agents/STATE.md 2>/dev/null | head -1)"
if [[ -n "$active" && -f "docs/agents/tasks/$active" ]]; then
    active_status="$(front_matter "docs/agents/tasks/$active" | sed -n 's/^status: *//p')"
    if [[ "$active_status" == "done" || "$active_status" == "abandoned" ]]; then
        fail "STATE.md names $active as active, but it is $active_status" "Point STATE.md at the next task."
        taskbad=1
    fi
fi
# Every task STATE.md cites by its slug must exist: a slug promises a file the next agent will look
# for (T-012). A bare ID (`T-003`) may name a task not yet written; a branch (`feat/T-…`) is not a task.
while read -r cited; do
    [[ -f "docs/agents/tasks/${cited#docs/agents/tasks/}.md" ]] && continue
    fail "STATE.md cites $cited, but there is no such task file" \
         "Write the task file, or cite the bare ID until it exists."
    taskbad=1
done < <(grep -oP '(?<![\w/-])(?:docs/agents/tasks/)?T-[0-9]{3}[a-z]?-[a-z0-9]+(?:-[a-z0-9]+)*' \
    docs/agents/STATE.md 2>/dev/null | sort -u)
[[ $taskbad -eq 0 ]] && ok "task front matter valid, done means done, parents track their slices, active task is open, cited tasks exist"

# ── Lifecycle matrix coverage (PLATFORM_AUDIO.md §5, T-002d) ─────────────────
# The M1 exit criterion is "every row of the lifecycle matrix". Each row says how its Tuner
# behaviour is proven, and this holds the column to it: a patrol row needs a test that names it, a
# manual row a device, an OS and a date, an open row a task still open, a deferred row a milestone.
printf '\n%sLifecycle matrix%s\n' "$BOLD" "$OFF"
matrix=docs/PLATFORM_AUDIO.md
lifecycle_test=apps/diapason/patrol_test/lifecycle_test.dart
# The ID pattern proves itself on a known test name first, as the TODO pattern does (T-010).
test_ids='^\s*(?:patrolTest\()?'"'"'\KL[0-9]+(?: L[0-9]+)*(?=:)'
if [[ "$(grep -oP "$test_ids" <<<"  patrolTest('L1 L2: home', (\$) {")" != "L1 L2" ]]; then
    fail "the lifecycle test-name pattern no longer extracts IDs" "Fix test_ids in tools/docs-check.sh."
fi
covered=" $(grep -oP "$test_ids" "$lifecycle_test" 2>/dev/null | tr '\n' ' ') "
matrixbad=0
rows=0
declare -A seen=()
while IFS='|' read -r _ id _ _ _ proof _; do
    id="$(xargs <<<"$id")"; proof="$(xargs <<<"$proof")"
    [[ "$id" =~ ^L[0-9]+$ ]] || continue
    rows=$((rows + 1))
    if [[ -n "${seen[$id]:-}" ]]; then
        fail "lifecycle row $id appears twice" "IDs are permanent: give the new row the next number."
        matrixbad=1
    fi
    seen[$id]=1
    case "$proof" in
        patrol)
            if [[ "$covered" != *" $id "* ]]; then
                fail "lifecycle row $id says patrol, but no test in $lifecycle_test starts with it" \
                     "Name the test '$id: …', or say how the row is verified instead."
                matrixbad=1
            fi ;;
        manual:\ *,\ *,\ [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) ;;
        open:\ T-[0-9][0-9][0-9]*)
            owner="${proof#open: }"
            file="$(compgen -G "docs/agents/tasks/${owner}-*.md" | head -1)"
            status="$([[ -n "$file" ]] && front_matter "$file" | sed -n 's/^status: *//p')"
            if [[ -z "$file" ]]; then
                fail "lifecycle row $id is owed by $owner, which has no task file" "Write the task first."
                matrixbad=1
            elif [[ "$status" == "done" || "$status" == "abandoned" ]]; then
                fail "lifecycle row $id is still open, but $owner is $status" \
                     "Verify the row (patrol or manual), or hand it to an open task."
                matrixbad=1
            fi ;;
        deferred:\ M[0-9])
            grep -q "^## ${proof#deferred: } " docs/ROADMAP.md || {
                fail "lifecycle row $id is deferred to ${proof#deferred: }, which ROADMAP.md lacks"
                matrixbad=1
            } ;;
        *)
            fail "lifecycle row $id: '$proof' is not patrol, manual: <device>, <OS>, <date>, open: T-###, or deferred: M#"
            matrixbad=1 ;;
    esac
done < <(awk '/^## 5\. Lifecycle matrix/ {f=1; next} f && /^## / {exit} f' "$matrix")
for id in $covered; do
    [[ -n "${seen[$id]:-}" ]] && continue
    fail "$lifecycle_test names $id, which is not a row of the matrix"
    matrixbad=1
done
if [[ $rows -eq 0 ]]; then
    fail "no rows read from the lifecycle matrix in $matrix" "Its heading or table layout changed."
elif [[ $matrixbad -eq 0 ]]; then
    ok "$rows lifecycle rows, each proven, owed by an open task, or deferred to a milestone"
fi

# ── Lint suppressions (AGENTS.md §3.2, adr/0018) ─────────────────────────────
# The gate means nothing if a lint can be switched off where it fires. The single audited home for
# unavoidable lossy casts is dsp/src/convert.rs; generated and vendored code is not ours to judge.
printf '\n%sLint suppressions%s\n' "$BOLD" "$OFF"
suppressed=0
while IFS= read -r hit; do
    fail "lint suppressed; fix the cause (adr/0018 names the one exception)" "$hit"
    suppressed=1
done < <(
    git grep -n -E '#!?\[(allow|expect)\(|cfg_attr\(.*\b(allow|expect)\(' -- '*.rs' \
        ':!rust/crates/dsp/src/convert.rs' ':!**/frb_generated*.rs' 2>/dev/null
    # A [lints] level of "allow" switches a lint off for a whole crate: the option adr/0018 rejected.
    git grep -n -E '"(allow|expect)"' -- '*Cargo.toml' ':!**/cargokit/**' 2>/dev/null
    git grep -n -E '//[[:space:]]*ignore(_for_file)?:' -- '*.dart' \
        ':!*.g.dart' ':!*.freezed.dart' ':!*frb_generated*' \
        ':!packages/audio_engine/lib/src/rust/**' ':!packages/audio_engine/cargokit/**' 2>/dev/null
)
[[ $suppressed -eq 0 ]] && ok "no #[allow]/#[expect] or // ignore: outside the audited module"

# ── Task IDs referenced in code must exist ───────────────────────────────────
printf '\n%sTask references%s\n' "$BOLD" "$OFF"
# A broken pattern matches nothing and passes silently, which is how this check was dead until
# T-010. So the pattern proves itself on a known line before it is trusted on the tree.
todo_id='TODO\(\K[^)]*'
if [[ "$(grep -oP "$todo_id" <<<'// TODO(T-123): x')" != "T-123" ]]; then
    fail "the TODO pattern no longer extracts a task ID" "Fix todo_id in tools/docs-check.sh."
fi
# `git grep -P` needs a git built with PCRE (Apple's is not). Exit 1 only means "no match"; anything
# above it means the scan never ran, which must not read as a pass. Untracked files are scanned
# too, so a local verify sees a new file before CI does.
unknown=0
todo_hits="$(git grep --untracked -h -o -P "$todo_id" -- '*.dart' '*.rs' '*.kt' '*.kts' '*.swift' \
    ':!**/cargokit/**' ':!**/frb_generated*' 2>&1)"
todo_rc=$?
if [[ $todo_rc -gt 1 ]]; then
    fail "the TODO scan could not run" "$todo_hits"
    unknown=1
    todo_hits=''
fi
while read -r id; do
    [[ -z "$id" ]] && continue
    # T-0xx is the documented placeholder for "a task that does not exist yet".
    [[ "$id" == "T-0xx" ]] && continue
    if [[ ! "$id" =~ ^T-[0-9]{3}[a-z]?$ ]] || ! compgen -G "docs/agents/tasks/${id}-*.md" >/dev/null; then
        fail "TODO references unknown task '$id'" "Create the task file, or use T-0xx for 'not yet scheduled'."
        unknown=1
    fi
done < <(sort -u <<<"$todo_hits")
[[ $unknown -eq 0 ]] && ok "every TODO names a real task (or T-0xx)"

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ %d documentation problem(s)%s\n\n' "$RED" "$FAILURES" "$OFF"; exit 1
fi
printf '%s✓ docs consistent%s\n\n' "$GREEN" "$OFF"

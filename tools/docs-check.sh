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
active="$(grep -oP 'docs/agents/tasks/\KT-[0-9a-z-]+\.md' docs/agents/STATE.md 2>/dev/null | head -1)"
if [[ -n "$active" && -f "docs/agents/tasks/$active" ]]; then
    active_status="$(front_matter "docs/agents/tasks/$active" | sed -n 's/^status: *//p')"
    if [[ "$active_status" == "done" || "$active_status" == "abandoned" ]]; then
        fail "STATE.md names $active as active, but it is $active_status" "Point STATE.md at the next task."
        taskbad=1
    fi
fi
[[ $taskbad -eq 0 ]] && ok "task front matter valid, done means done, active task is open"

# ── Task IDs referenced in code must exist ───────────────────────────────────
printf '\n%sTask references%s\n' "$BOLD" "$OFF"
unknown=0
while read -r id; do
    [[ -z "$id" ]] && continue
    # T-0xx is the documented placeholder for "a task that does not exist yet".
    [[ "$id" == "T-0xx" ]] && continue
    compgen -G "docs/agents/tasks/${id}-*.md" >/dev/null || {
        fail "TODO references unknown task $id" "Create the task file, or use T-0xx for 'not yet scheduled'."
        unknown=1; }
done < <(grep -rhoP 'TODO\(\K T?-?[0-9A-Za-z]+' --include='*.dart' --include='*.rs' --include='*.kts' . 2>/dev/null | sort -u)
[[ $unknown -eq 0 ]] && ok "every TODO names a real task (or T-0xx)"

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ %d documentation problem(s)%s\n\n' "$RED" "$FAILURES" "$OFF"; exit 1
fi
printf '%s✓ docs consistent%s\n\n' "$GREEN" "$OFF"

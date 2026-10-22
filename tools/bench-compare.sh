#!/usr/bin/env bash
# Compare Criterion results against the committed baseline; fail on a >10% regression.
#
#   tools/bench-compare.sh          compare the latest results against the baseline
#   tools/bench-compare.sh --save   run every benchmark afresh and record the run as the baseline
#
# `just bench` runs `cargo bench -p diapason_dsp`, then this. Criterion leaves each benchmark's
# latest estimates in <target>/criterion/<group>/<id>/new/estimates.json; the figure compared is
# the median (`.median.point_estimate`, in nanoseconds), which an outlier on a busy machine moves
# less than the mean.
#
# The baseline, rust/crates/dsp/benches/baseline.json, records the median of every benchmark with
# the host it was measured on (`uname -sm` and the CPU model, not the kernel release, which changes
# with every OS update), the rustc version and the date. Absolute timings only compare on the same
# machine: record a baseline with `just bench-baseline` and commit it with the change that moved
# it. A run on another host, or with another rustc, is still compared under a warning, because a
# regression that large is worth a look anywhere.
#
# `--save` (`just bench-baseline`) runs the full `cargo bench -p diapason_dsp` itself and adopts only
# results that run wrote. Results left in <target>/criterion by a renamed or removed benchmark, or
# by an earlier filtered run, are listed and ignored, so they can never enter a baseline.
#
# Every benchmark in the baseline must have a result; one without is an error (renamed or not
# run). A result with no baseline entry is reported and does not fail: `--save` adopts it. After
# `just bench <filter>`, unfiltered benchmarks show their previous result, which is still valid.
#
# Exit codes: 0 no regression (or baseline saved), 1 a regression or a missing result, 2 cannot
# compare (no results, no or unreadable baseline, unreadable estimates, no jq, a failed bench run).

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 2

BASELINE="rust/crates/dsp/benches/baseline.json"
THRESHOLD_PERCENT=10

cannot() {
    echo "✗ $1" >&2
    [[ $# -gt 1 ]] && echo "  $2" >&2
    exit 2
}

command -v jq >/dev/null 2>&1 || cannot "jq is not on PATH; it reads Criterion's JSON (https://jqlang.org)"

target="$(cargo metadata --format-version 1 --no-deps 2>/dev/null | jq -r '.target_directory')"
[[ -n "$target" && "$target" != "null" ]] || cannot "cargo metadata did not name a target directory"
criterion="$target/criterion"

# Every result as one JSON object, {"group/id": median_ns, ...}, keys sorted. With a file as $1,
# only results written after it; the others are listed on stderr as ignored. Exits 2 on a result
# whose estimates are not a number.
current_results() {
    local newer="${1:-}" file key median first=1
    printf '{'
    while IFS= read -r file; do
        key="${file#"$criterion"/}"
        key="${key%/new/estimates.json}"
        if [[ -n "$newer" && ! "$file" -nt "$newer" ]]; then
            echo "  ignored, not from this run: $key" >&2
            continue
        fi
        median="$(jq -e '.median.point_estimate | numbers' "$file" 2>/dev/null)" ||
            cannot "unreadable estimates: $file" "Re-run the benchmarks: \`just bench\`."
        [[ $first -eq 1 ]] || printf ','
        first=0
        printf '%s:%s' "$(jq -n --arg k "$key" '$k')" "$median"
    done < <(find "$criterion" -path '*/new/estimates.json' -type f 2>/dev/null | LC_ALL=C sort)
    printf '}'
}

host_description() {
    local cpu=""
    if [[ -r /proc/cpuinfo ]]; then
        cpu="$(sed -n 's/^model name[[:space:]]*:[[:space:]]*//p' /proc/cpuinfo | head -n 1)"
    elif command -v sysctl >/dev/null 2>&1; then
        cpu="$(sysctl -n machdep.cpu.brand_string 2>/dev/null)"
    fi
    if [[ -n "$cpu" ]]; then
        printf '%s; %s' "$(uname -sm)" "$cpu"
    else
        uname -sm
    fi
}

if [[ "${1:-}" == "--save" ]]; then
    mkdir -p "$target" || cannot "cannot create $target"
    # Script-level, not local: the EXIT trap runs after everything else.
    marker="$(mktemp "$target/bench-baseline.XXXXXX")" || cannot "cannot create a marker in $target"
    trap 'rm -f "${marker:-}"' EXIT
    cargo bench -p diapason_dsp || cannot "cargo bench failed; no baseline saved"
    results="$(current_results "$marker")" || exit 2
    [[ "$results" != "{}" ]] || cannot "the bench run wrote no results; no baseline saved"
    jq -n \
        --arg host "$(host_description)" \
        --arg rustc "$(rustc --version)" \
        --arg date "$(date -u +%Y-%m-%d)" \
        --argjson medians "$results" \
        '{host: $host, rustc: $rustc, date: $date, unit: "ns (median)", benches: $medians}' \
        >"$BASELINE" || cannot "could not write $BASELINE"
    echo "✓ baseline saved to $BASELINE ($(jq '.benches | length' "$BASELINE") benches)"
    exit 0
fi

results="$(current_results)" || exit 2
[[ "$results" != "{}" ]] || cannot "no Criterion results under $criterion; run \`just bench\` first"

[[ -f "$BASELINE" ]] || cannot "no baseline at $BASELINE" \
    "Record one on this machine with \`just bench-baseline\`, and commit it."
jq -e '(.host | strings) and (.rustc | strings)
       and (.benches | type == "object" and length > 0 and all(.[]; type == "number"))' \
    "$BASELINE" >/dev/null 2>&1 ||
    cannot "unreadable baseline: $BASELINE" "Re-record it with \`just bench-baseline\`."

baseline_host="$(jq -r '.host' "$BASELINE")"
this_host="$(host_description)"
if [[ "$baseline_host" != "$this_host" ]]; then
    echo "⚠ baseline measured on: $baseline_host"
    echo "  this host:             $this_host"
    echo "  timings across machines are not comparable; read the table, not just the verdict."
fi
baseline_rustc="$(jq -r '.rustc' "$BASELINE")"
this_rustc="$(rustc --version)"
if [[ "$baseline_rustc" != "$this_rustc" ]]; then
    echo "⚠ baseline built with: $baseline_rustc"
    echo "  this build:          $this_rustc"
    echo "  a compiler change moves timings; re-record the baseline if it is the cause."
fi

# One row per benchmark in either set: name|baseline|current, a median empty when absent.
# Not tab-separated: tab is IFS whitespace, so `read` would merge an empty field into the next.
rows="$(jq -r -n --slurpfile base "$BASELINE" --argjson now "$results" '
    ($base[0].benches) as $b
    | (($b | keys) + ($now | keys) | unique)[]
    | [., ($b[.] // "" | tostring), ($now[.] // "" | tostring)]
    | join("|")')" || cannot "could not compare $BASELINE with the results"
[[ -n "$rows" ]] || cannot "nothing to compare"

failed=0
printf '%-24s %14s %14s %9s\n' "benchmark" "baseline µs" "current µs" "change"
while IFS='|' read -r name base now; do
    if [[ -z "$now" ]]; then
        printf '%-24s %14s %14s %9s  ✗ no result\n' "$name" \
            "$(awk -v b="$base" 'BEGIN { printf "%.1f", b / 1000 }')" "-" "-"
        failed=1
    elif [[ -z "$base" ]]; then
        printf '%-24s %14s %14s %9s  not in baseline\n' "$name" "-" \
            "$(awk -v n="$now" 'BEGIN { printf "%.1f", n / 1000 }')" "-"
    else
        verdict="$(awk -v b="$base" -v n="$now" -v t="$THRESHOLD_PERCENT" 'BEGIN {
            change = (n - b) / b * 100
            printf "%.1f %.1f %+.1f%% %s", b / 1000, n / 1000, change, (change > t ? "slower" : "ok")
        }')"
        read -r base_us now_us change status <<<"$verdict"
        if [[ "$status" == "slower" ]]; then
            printf '%-24s %14s %14s %9s  ✗ over %d%%\n' "$name" "$base_us" "$now_us" "$change" \
                "$THRESHOLD_PERCENT"
            failed=1
        else
            printf '%-24s %14s %14s %9s\n' "$name" "$base_us" "$now_us" "$change"
        fi
    fi
done <<<"$rows"

if [[ $failed -ne 0 ]]; then
    echo "✗ benchmark regression or missing result (baseline: $BASELINE)"
    exit 1
fi
echo "✓ no benchmark more than ${THRESHOLD_PERCENT}% slower than the baseline"

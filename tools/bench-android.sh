#!/usr/bin/env bash
# Run dsp's criterion benches on a connected Android device, on each CPU cluster (T-026).
#
#   tools/bench-android.sh [--save] [--runs N]
#
# The binaries are pushed by hand rather than through cargo-ndk-runner, because the runner cannot
# pin a process to a core. Nothing pins the analysis thread either, so the OS may run it on a little
# core: the all-cores median alone would flatter the budgets in docs/AUDIO_ENGINE.md §1. Each set
# (all cores, then each cluster by cpuinfo_max_freq) runs N times, and each bench records its median
# of the runs. Cargo's bench profile inherits release, so this is the shipped code generation.
#
# --save writes rust/crates/dsp/benches/baseline-redmi.json. Thermal status, the skin temperature and
# wakefulness are read around every run, at its edges only, so a brief throttle between them can hide.
# ANDROID_SERIAL picks the device. Exit: 0 ok, 1 a budget failed, 2 could not measure.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 2
# shellcheck disable=SC1091
source tools/versions.env

BASELINE=rust/crates/dsp/benches/baseline-redmi.json
REMOTE=/data/local/tmp/diapason-bench
# docs/AUDIO_ENGINE.md §1, the frame-cost and CPU rows.
FRAME_TARGET_NS=1500000 FRAME_FAIL_NS=4000000
CPU_TARGET_PERCENT=8 CPU_FAIL_PERCENT=15
# chain/analysis, chord and strum each take one 10 ms block (benches/chain.rs).
BLOCK_NS=10000000

cannot() { echo "✗ $1" >&2; exit 2; }

save=0 runs=3
while [[ $# -gt 0 ]]; do
    case "$1" in
        --save) save=1 ;;
        --runs) runs="${2:-}"; shift ;;
        *) cannot "unknown argument: $1" ;;
    esac
    shift
done
[[ "$runs" =~ ^[1-9][0-9]*$ ]] || cannot "--runs needs a positive number"
adb get-state >/dev/null 2>&1 || cannot "no Android device reachable over adb (set ANDROID_SERIAL)"
command -v jq >/dev/null 2>&1 || cannot "jq is not on PATH"

export ANDROID_NDK_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/$ANDROID_NDK_VERSION"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

echo "── building the benches (arm64-v8a, API $ANDROID_MIN_SDK, $CARGO_BUILD_JOBS jobs)"
# `cargo ndk` swallows cargo's JSON, which names the binaries, and its exported environment does not
# link outside it. The NDK's own clang driver is all a link needs, and criterion's `alloca` needs it as
# a C compiler. Nothing C reaches the code being measured: dsp itself has none.
host_tag="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
ndk_bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host_tag/bin"
clang="$ndk_bin/aarch64-linux-android$ANDROID_MIN_SDK-clang"
[[ -x "$clang" ]] || cannot "no NDK clang at $clang (just doctor)"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$clang" CC_aarch64_linux_android="$clang"
export AR_aarch64_linux_android="$ndk_bin/llvm-ar"
build="$(cargo bench --target aarch64-linux-android -p diapason_dsp --no-run \
    --message-format=json)" || cannot "the build failed"
mapfile -t binaries < <(jq -r 'select(.reason == "compiler-artifact" and (.target.kind | index("bench")))
    | .executable // empty' <<<"$build")
[[ ${#binaries[@]} -gt 0 ]] || cannot "the build named no bench binaries"

work="$(mktemp -d)"
trap 'rm -r -- "$work"; adb shell rm -r "$REMOTE" >/dev/null 2>&1' EXIT
adb shell "rm -r $REMOTE; mkdir -p $REMOTE" >/dev/null 2>&1
names=()
for binary in "${binaries[@]}"; do
    # cargo names it nsdf-<hash>; the name on the device does not matter.
    name="$(basename "$binary")"
    adb push "$binary" "$REMOTE/$name" >/dev/null || cannot "could not push $name"
    adb shell chmod +x "$REMOTE/$name"
    names+=("$name")
done

# Cluster masks: every core, grouped by its maximum frequency, slowest first.
# shellcheck disable=SC2016 # expanded by the device's shell, not this one
freqs="$(adb shell 'for c in /sys/devices/system/cpu/cpu[0-9]*; do
    echo "${c##*cpu} $(cat "$c/cpufreq/cpuinfo_max_freq")"; done' | tr -d '\r')"
[[ -n "$freqs" ]] || cannot "could not read the cores' frequencies"
sets=(all) masks=(-)
while read -r freq; do
    mask=0
    while read -r core; do mask=$((mask | (1 << core))); done \
        < <(awk -v f="$freq" '$2 == f { print $1 }' <<<"$freqs")
    sets+=("cluster-$freq"); masks+=("$(printf '%x' "$mask")")
done < <(awk '{ print $2 }' <<<"$freqs" | sort -un)
little="${sets[1]}"
echo "── sets: ${sets[*]} (masks ${masks[*]}), $runs run(s) each"

# shellcheck disable=SC2016 # expanded by the device's shell, not this one
vitals() {
    # Only the status and the HAL's current skin reading: on the Redmi the dump's "Cached temperatures"
    # never move, and the HAL reports CPU as -274 (no sensor). Skin is what its throttling follows.
    adb shell 'dumpsys thermalservice > /data/local/tmp/diapason-bench/thermal.txt
        grep -m1 "Thermal Status" /data/local/tmp/diapason-bench/thermal.txt | tr -d "\t"
        sed -n "/Current temperatures from HAL/,\$p" /data/local/tmp/diapason-bench/thermal.txt \
            | grep -m1 "mName=SKIN" | grep -oE "mValue=[0-9.]+" | sed "s/mValue=/skin=/"
        dumpsys power | grep mWakefulness= | head -1 | tr -d " "' \
        | tr -d '\r' | tr '\n' ' '
}

results="$work/results.tsv"
log="$work/vitals.txt"
for i in "${!sets[@]}"; do
    set_name="${sets[$i]}" mask="${masks[$i]}"
    prefix=''
    [[ "$mask" != - ]] && prefix="taskset $mask "
    for ((run = 1; run <= runs; run++)); do
        home="$REMOTE/criterion"
        before="$(vitals)"
        echo "   $set_name run $run/$runs: $before"
        for name in "${names[@]}"; do
            adb shell "rm -r $home 2>/dev/null; mkdir -p $home; cd $REMOTE && CRITERION_HOME=$home \
                ${prefix}./$name --bench --warm-up-time 1 --measurement-time 3 --sample-size 50" \
                >"$work/last.log" 2>&1 || { cat "$work/last.log" >&2; cannot "$name failed on the device"; }
            [[ -d "$work/pulled" ]] && rm -r -- "$work/pulled"
            adb pull "$home" "$work/pulled" >/dev/null 2>&1 || cannot "could not pull criterion's results"
            while IFS= read -r estimates; do
                id="${estimates#"$work/pulled/"}"
                id="${id%/new/estimates.json}"
                printf '%s\t%s\t%s\n' "$set_name" "$id" \
                    "$(jq '.median.point_estimate' "$estimates")" >>"$results"
            done < <(find "$work/pulled" -path '*/new/estimates.json')
        done
        printf '%s run %d: %s -> %s\n' "$set_name" "$run" "$before" "$(vitals)" >>"$log"
    done
done

# Each bench's median of the runs: the upper median, which only matters for an even --runs.
medians="$(jq -Rn '
    [inputs | split("\t") | {set: .[0], id: .[1], ns: (.[2] | tonumber)}]
    | group_by(.set) | map({key: .[0].set, value: (group_by(.id) | map({key: .[0].id,
        value: (map(.ns) | sort | .[length / 2 | floor])}) | from_entries)}) | from_entries
    ' "$results")"

echo
printf '%-22s' bench; for s in "${sets[@]}"; do printf '%18s' "$s"; done; echo ' (µs, median)'
while read -r id; do
    printf '%-22s' "$id"
    for s in "${sets[@]}"; do
        printf '%18s' "$(jq -r --arg s "$s" --arg id "$id" \
            '.[$s][$id] // empty | . / 1000 | . * 10 | round | . / 10' <<<"$medians")"
    done
    echo
done < <(jq -r '.[] | keys[]' <<<"$medians" | sort -u)

# The verdict is the slowest cluster's: the worst core the OS may hand the analysis thread.
echo
echo "── verdict on $little (AUDIO_ENGINE.md §1)"
status=0
judge() { # LABEL VALUE TARGET FAIL UNIT
    local verdict=ok
    if awk -v v="$2" -v f="$4" 'BEGIN { exit !(v > f) }'; then verdict=FAIL; status=1
    elif awk -v v="$2" -v t="$3" 'BEGIN { exit !(v > t) }'; then verdict='over target'; fi
    printf '  %-24s %10.2f %-3s target %s, fails at %s: %s\n' "$1" "$2" "$5" "$3" "$4" "$verdict"
}
for id in chain/frame/low chain/frame/mid chain/frame/high nsdf/4096; do
    ns="$(jq -r --arg s "$little" --arg id "$id" '.[$s][$id] // empty' <<<"$medians")"
    [[ -n "$ns" ]] || cannot "no result for $id"
    judge "$id" "$(awk -v n="$ns" 'BEGIN { print n / 1000000 }')" \
        "$(awk -v n="$FRAME_TARGET_NS" 'BEGIN { print n / 1000000 }')" \
        "$(awk -v n="$FRAME_FAIL_NS" 'BEGIN { print n / 1000000 }')" ms
done
for id in chain/analysis chain/chord chain/strum; do
    ns="$(jq -r --arg s "$little" --arg id "$id" '.[$s][$id] // empty' <<<"$medians")"
    [[ -n "$ns" ]] || cannot "no result for $id"
    judge "$id" "$(awk -v n="$ns" -v b="$BLOCK_NS" 'BEGIN { print 100 * n / b }')" \
        "$CPU_TARGET_PERCENT" "$CPU_FAIL_PERCENT" %
done

echo
echo "── vitals (thermal status, skin °C, wakefulness)"
cat "$log"
if grep -qE 'Thermal Status: [1-9]' "$log"; then
    echo "⚠ the device reported thermal pressure during these runs: treat the figures with care"
fi

if [[ $save -eq 1 ]]; then
    model="$(adb shell getprop ro.product.model | tr -d '\r')"
    # The file is the reference device's (adr/0035). Another phone's figures are worth reading, not keeping.
    [[ "$model" == 23117RA68G ]] || cannot "--save is for the Redmi 23117RA68G; this is $model"
    jq -n --argjson benches "$medians" --arg rustc "$(rustc --version)" --arg date "$(date +%F)" \
        --arg model "$model" \
        --arg soc "$(adb shell getprop ro.soc.model | tr -d '\r')" \
        --arg android "$(adb shell getprop ro.build.version.release | tr -d '\r')" \
        --arg little "$little" --argjson runs "$runs" --rawfile vitals "$log" \
        '{device: $model, soc: $soc, android: $android, rustc: $rustc, date: $date,
          unit: "ns (median of runs)", runs: $runs, verdict_set: $little,
          vitals: ($vitals | split("\n") | map(select(length > 0))), benches: $benches}' \
        >"$work/baseline.json" || cannot "could not write the baseline"
    mv "$work/baseline.json" "$BASELINE" || cannot "could not write $BASELINE"
    echo "✓ saved $BASELINE"
fi
exit "$status"

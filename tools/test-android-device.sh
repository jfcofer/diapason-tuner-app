#!/usr/bin/env bash
# Run audio_io's on-device tests on a connected Android device over adb (T-002b).
#
# Builds the test binaries for arm64-v8a at the minSdk with the pinned NDK, pushes them to the
# device and runs them as the shell user, which holds RECORD_AUDIO. Two verdicts:
#   - android_conformance must pass: AAudioBackend honours the AudioBackend contract on hardware;
#   - android_alloc_canary must die of SIGABRT: proof the allocation trap is armed, so the clean
#     conformance run means the audio thread really never allocated.
#
# Usage: tools/test-android-device.sh [adb-serial]. CARGO_BUILD_JOBS defaults to 4, because a cold
# build at full parallelism has exhausted the 14 GB dev host (docs/agents/STATE.md, Traps).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
# shellcheck disable=SC1091
source tools/versions.env

serial="${1:-${ANDROID_SERIAL:-}}"
adb=(adb)
[[ -n "$serial" ]] && adb+=(-s "$serial")
if ! "${adb[@]}" get-state >/dev/null 2>&1; then
    echo "✗ no Android device reachable over adb (pass a serial, or set ANDROID_SERIAL)" >&2
    exit 1
fi

export ANDROID_NDK_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/$ANDROID_NDK_VERSION"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "✗ pinned NDK $ANDROID_NDK_VERSION not found at $ANDROID_NDK_HOME (just doctor)" >&2
    exit 1
fi

echo "Building audio_io device tests (arm64-v8a, API $ANDROID_MIN_SDK, $CARGO_BUILD_JOBS jobs)"
cargo ndk -t arm64-v8a -P "$ANDROID_MIN_SDK" test -p diapason_audio_io --features conformance --no-run
# cargo-ndk swallows both --message-format=json and cargo's "Executable" lines, so take the newest
# build of each device test from the target directory: the one just built.
deps=target/aarch64-linux-android/debug/deps
binaries=""
for name in android_conformance android_alloc_canary; do
    newest="$(find "$deps" -maxdepth 1 -type f -perm -u+x -name "$name-*" -printf '%T@ %p\n' \
        | sort -n | tail -1 | cut -d' ' -f2)"
    [[ -n "$newest" ]] && binaries+="$name $newest"$'\n'
done
binaries="${binaries%$'\n'}"
if [[ -z "$binaries" ]]; then
    echo "✗ no device test binaries were built" >&2
    exit 1
fi

dir=/data/local/tmp/diapason-audio-io
exit_file="$(mktemp)"
trap 'rm -f "$exit_file"' EXIT
"${adb[@]}" shell "mkdir -p $dir"
failures=0
while read -r name path; do
    "${adb[@]}" push "$path" "$dir/$name" >/dev/null </dev/null
    "${adb[@]}" shell -n "chmod 755 $dir/$name"
    echo
    echo "── $name"
    set +e
    # --nocapture shows report_what_the_device_grants; --test-threads=1 because the device has one
    # audio output, and parallel streams would measure each other.
    "${adb[@]}" shell -n "$dir/$name --test-threads=1 --nocapture; echo exit=\$?" | tee /dev/stderr \
        | grep -o 'exit=[0-9]*' | tail -1 > "$exit_file"
    set -e
    code="$(cut -d= -f2 < "$exit_file")"
    case "$name" in
        android_alloc_canary)
            # 134 = 128 + SIGABRT. Anything else means the trap did not fire.
            if [[ "$code" == 134 ]]; then
                echo "✓ canary aborted: the allocation trap is armed"
            else
                echo "✗ canary exited $code: the allocation trap is NOT armed"; failures=$((failures + 1))
            fi ;;
        *)
            if [[ "$code" == 0 ]]; then
                echo "✓ $name passed"
            else
                echo "✗ $name exited $code"; failures=$((failures + 1))
            fi ;;
    esac
done <<<"$binaries"

exit "$failures"

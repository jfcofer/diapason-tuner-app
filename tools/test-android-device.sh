#!/usr/bin/env bash
# Run audio_io's on-device tests on a connected Android device over adb (T-002b).
#
# Builds the tests for arm64-v8a at the minSdk with the pinned NDK. cargo-ndk's own runner
# (cargo-ndk-runner, installed with cargo-ndk 4) pushes the exact binary cargo built and runs it on
# the device as the shell user, which holds RECORD_AUDIO. It passes the exit status back, and
# honours ANDROID_SERIAL. Two verdicts:
#   - android_conformance must pass: AAudioBackend honours the AudioBackend contract on hardware;
#   - android_alloc_canary must die of SIGABRT with the allocator's message: proof the allocation
#     trap is armed, so the clean conformance run means Rust never allocated on the audio thread.
#
# Usage: tools/test-android-device.sh [--release] [adb-serial]
#   --release  measures callback timing without debug overhead. There is no allocation trap in
#              release builds, so the canary is skipped.
# CARGO_BUILD_JOBS defaults to 4: a cold build at full parallelism has exhausted the 14 GB dev host
# (docs/agents/STATE.md, Traps).

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
# shellcheck disable=SC1091
source tools/versions.env

profile=()
if [[ "${1:-}" == --release ]]; then
    profile=(--release)
    shift
fi
[[ -n "${1:-}" ]] && export ANDROID_SERIAL="$1"
if ! adb get-state >/dev/null 2>&1; then
    echo "✗ no Android device reachable over adb (pass a serial, or set ANDROID_SERIAL)" >&2
    exit 1
fi

export ANDROID_NDK_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/$ANDROID_NDK_VERSION"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
    echo "✗ pinned NDK $ANDROID_NDK_VERSION not found at $ANDROID_NDK_HOME (just doctor)" >&2
    exit 1
fi

# One test at a time: the device has one speaker, and parallel streams would measure each other.
device_test() {
    cargo ndk -t arm64-v8a -P "$ANDROID_MIN_SDK" test -p diapason_audio_io --features conformance \
        ${profile[@]+"${profile[@]}"} --test "$1" -- --test-threads=1 --nocapture
}

echo "── android_conformance (arm64-v8a, API $ANDROID_MIN_SDK, ${profile[*]:-debug}, $CARGO_BUILD_JOBS jobs)"
failures=0
if device_test android_conformance; then
    echo "✓ android_conformance passed"
else
    echo "✗ android_conformance failed"
    failures=$((failures + 1))
fi

if [[ ${#profile[@]} -eq 0 ]]; then
    echo
    echo "── android_alloc_canary"
    log="$(mktemp)"
    trap 'rm -f "$log"' EXIT
    set +e
    device_test android_alloc_canary 2>&1 | tee "$log"
    set -e
    # 134 = 128 + SIGABRT, and the message is the allocation trap's own: an abort from anything
    # else is not proof that the trap fired.
    if grep -q "memory allocation of" "$log" && grep -q "exit status: 134" "$log"; then
        echo "✓ canary aborted in the allocation trap: the trap is armed"
    else
        echo "✗ canary did not abort in the allocation trap: the trap is NOT proven armed"
        failures=$((failures + 1))
    fi
fi

exit "$failures"

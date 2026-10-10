#!/usr/bin/env bash
# The engine contract against the real Rust engine on a connected Android device (T-002b).
#
# Runs apps/diapason/integration_test/engine_roundtrip_test.dart twice:
#   1. without RECORD_AUDIO, so asking for the microphone must leave the output running and
#      report an input fault: the metronome never depends on the microphone;
#   2. with it, so the duplex stream opens.
# The test is told which to expect, and fails if the device did not keep the permission.
#
# Setting the permission: `pm revoke`/`pm grant` where the shell may change permissions. Some
# vendors forbid it, and `adb uninstall` too (Xiaomi HyperOS, unless "USB debugging (Security
# settings)" is on). There, the first run needs the permission off (a fresh install, or turned off
# in Settings), and the second shows the system prompt, which a person must allow on the device.

# The debug build installs the allocation trap as the global allocator, so any Rust allocation on
# the audio thread during either run aborts the app (docs/AUDIO_ENGINE.md §7).
#
# Usage: tools/test-integration-android.sh [adb-serial]

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

[[ -n "${1:-}" ]] && export ANDROID_SERIAL="$1"
if ! adb get-state >/dev/null 2>&1; then
    echo "✗ no Android device reachable over adb (pass a serial, or set ANDROID_SERIAL)" >&2
    exit 1
fi
serial="$(adb get-serialno)"
package=dev.jfcofer.diapason.dev
permission=android.permission.RECORD_AUDIO
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

run() {
    (cd apps/diapason && flutter test integration_test/engine_roundtrip_test.dart \
        --flavor dev -d "$serial" --dart-define=DIAPASON_MIC="$1")
}

# `pm` prints its SecurityException and still exits 0 on some builds, so judge it by its output.
pm_permission() {
    local out
    out="$(adb shell -n pm "$1" "$package" "$permission" 2>&1)"
    [[ -z "$out" ]]
}

echo "── without the microphone: output only, with an input fault"
if adb shell -n pm path "$package" >/dev/null 2>&1 && pm_permission revoke; then
    run denied
    echo "── with the microphone: duplex"
    pm_permission grant || { echo "✗ pm could revoke but not grant $permission" >&2; exit 1; }
    run granted
else
    echo "   this device does not let the shell change permissions. If the next run fails because"
    echo "   the microphone is allowed, turn it off for Diapason (Dev) in Settings and run again."
    run denied
    echo "── with the microphone: duplex"
    echo "   ▶ allow the microphone prompt on the device"
    run request
fi

echo "✓ the real engine honours the contract with and without the microphone"

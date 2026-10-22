#!/usr/bin/env bash
# The device tests in apps/diapason/patrol_test/ on a connected Android device (adr/0026): the
# engine contract against the real Rust engine, with the microphone denied and then allowed
# (T-002b), and the tuner's lifecycle rows of PLATFORM_AUDIO.md §5 (T-002d).
#
# Patrol clears the app's data before each test, so every test starts with the microphone
# undecided, and it answers the system dialog the way the test needs. No `pm grant`/`pm revoke`.
# A person is needed only where the vendor confirms installs: HyperOS asks for every USB install,
# at the start of the run.
#
# The debug build installs the allocation trap as the global allocator, so any Rust allocation on
# the audio thread during a test aborts the app (docs/AUDIO_ENGINE.md §7).
#
# Usage: tools/test-integration-android.sh [adb-serial]

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

[[ -n "${1:-}" ]] && export ANDROID_SERIAL="$1"
if ! adb get-state >/dev/null 2>&1; then
    echo "✗ no Android device reachable over adb (pass a serial, or set ANDROID_SERIAL)" >&2
    exit 1
fi
ANDROID_SERIAL="$(adb get-serialno)"
export ANDROID_SERIAL

# Patrol reports a refused install only as "Gradle test execution failed", so name the cause.
results=apps/diapason/build/app/outputs/androidTest-results
if ! tools/patrol.sh test; then   # every test in patrol_test/ (pubspec.yaml, `patrol:`)
    if grep -rqs INSTALL_FAILED_USER_RESTRICTED "$results"; then
        echo "✗ the device refused the install: approve the USB install prompts on it, and run again" >&2
    fi
    exit 1
fi
echo "✓ the real engine honours the contract, and the tuner releases the microphone off screen"

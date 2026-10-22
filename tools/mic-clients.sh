#!/usr/bin/env bash
# What the OS says about the app's microphone, from a connected Android device (T-002d): the
# evidence for the lifecycle rows of docs/PLATFORM_AUDIO.md §5, which the engine's own snapshot
# cannot give (the engine reports what it asked for, not what the OS still holds).
#
# Read-only adb. Two sources, both formats measured on the Redmi (Android 16) before writing this:
#   - `dumpsys media.audio_policy`, the "AudioRecord Clients" lists under " Inputs (N):": the
#     capture clients open right now, "<n>. Port ID: …; Session ID: …; uid <uid>; State: Active".
#     The "Outputs" section lists AudioTrack clients in the same format, so only Inputs counts;
#   - `dumpsys audio`, the recording-activity log: "MM-DD HH:MM:SS:mmm rec <event> riid:… uid:…
#     session:… src:… [not ]silenced pack:<package>", timestamped by the device, so a release can
#     be timed against the key event that caused it.
#
# Usage: tools/mic-clients.sh [package]        default: the dev flavour
#        ANDROID_SERIAL picks the device.
# Exit:  0 when the app holds no capture client, 3 when it holds one, 1 on error.

set -euo pipefail

package="${1:-dev.jfcofer.diapason.dev}"

if ! adb get-state >/dev/null 2>&1; then
    echo "✗ no Android device reachable over adb (set ANDROID_SERIAL)" >&2
    exit 1
fi
uid="$(adb shell pm list packages -U "$package" | tr -d '\r' \
    | sed -n "s/^package:${package//./\\.} uid:\([0-9]*\)$/\1/p")"
if [[ -z "$uid" ]]; then
    echo "✗ $package is not installed" >&2
    exit 1
fi

policy="$(adb shell dumpsys media.audio_policy | tr -d '\r')"
# A parse that finds no Inputs section is a changed format, not a released microphone.
if ! grep -qE '^ Inputs \([0-9]+\):' <<<"$policy"; then
    echo "✗ dumpsys media.audio_policy has no Inputs section: its format changed" >&2
    exit 1
fi
# From " Inputs (N):" to the next top-level section, the app's AudioRecord clients only.
clients="$(awk -v uid="uid ${uid};" '
    /^ Inputs \([0-9]+\):/ { f = 1; next }
    f && /^ [A-Z]/            { exit }
    f && /AudioRecord Clients/ { r = 1; next }
    f && /Clients \(|clients \(/ { r = 0 }
    r && index($0, uid) && /Session ID/' <<<"$policy")"

echo "$package (uid $uid), device time $(adb shell "date '+%m-%d %H:%M:%S'" | tr -d '\r')"
echo "Recent recording events:"
adb shell dumpsys audio | tr -d '\r' | grep -E " rec [a-z]+ riid:[0-9]+ uid:${uid} " | tail -6 \
    | sed 's/^/  /' || echo "  none"
if [[ -n "$clients" ]]; then
    echo "✗ capture client open:"
    while read -r client; do echo "  $client"; done <<<"$clients"
    exit 3
fi
echo "✓ no capture client"

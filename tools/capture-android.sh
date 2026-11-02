#!/usr/bin/env bash
# Record the microphone on an attached Android device, outside the repo (T-026).
#
#   tools/capture-android.sh take <name> [seconds]    one take
#   tools/capture-android.sh session                  T-026's whole list, with a redo after each
#
# Takes go to DIAPASON_CAPTURE_DIR, by default ~/.local/share/diapason/captures/<date>/. The repo is
# public and a phone hears the room, so nothing lands in it until the owner has listened to a cut
# and imported it (docs/adr/0030). A take is never overwritten: a repeated name gets a suffix.
# Beside each WAV, a .txt holds what the stream reported. ANDROID_SERIAL picks the device.
# Exit: 0 kept, 1 the take was rejected or could not be made. What a rejection can and cannot catch is
# in the example's own header (rust/crates/audio_io/examples/capture.rs).

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
# shellcheck disable=SC1091
source tools/versions.env

REMOTE=/data/local/tmp/diapason-capture.wav
DIR="${DIAPASON_CAPTURE_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/diapason/captures/$(date +%F)}"
export ANDROID_NDK_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}/ndk/$ANDROID_NDK_VERSION"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

adb get-state >/dev/null 2>&1 || { echo "✗ no Android device reachable over adb" >&2; exit 1; }

# The first free path for a name: e2.wav, then e2-2.wav.
free_path() {
    local path="$DIR/$1.wav" n=2
    while [[ -e "$path" ]]; do path="$DIR/$1-$n.wav"; n=$((n + 1)); done
    printf '%s' "$path"
}

take() { # NAME SECONDS
    local name="$1" seconds="$2" path log status
    [[ "$name" =~ ^[a-z0-9][a-z0-9_-]*$ ]] || { echo "✗ a take's name is lower-case letters, digits, - and _" >&2; return 1; }
    [[ "$seconds" =~ ^[1-9][0-9]*$ ]] || { echo "✗ seconds must be a positive number" >&2; return 1; }
    mkdir -p "$DIR" || return 1
    path="$(free_path "$name")"
    log="${path%.wav}.txt"
    cargo ndk -t arm64-v8a -P "$ANDROID_MIN_SDK" run --release --quiet -p diapason_audio_io \
        --example capture -- "$REMOTE" "$seconds" 2>&1 | tee "$log"
    status="${PIPESTATUS[0]}"
    if [[ "$status" -ne 0 ]]; then
        rm -f "$log"
        adb shell rm -f "$REMOTE" >/dev/null 2>&1
        echo "✗ $name rejected: nothing kept" >&2
        return 1
    fi
    adb pull "$REMOTE" "$path" >/dev/null 2>&1 || { echo "✗ could not pull the take" >&2; return 1; }
    adb shell rm -f "$REMOTE" >/dev/null 2>&1
    echo "✓ kept $path"
}

# name | seconds | what to do. The order makes each take cheap to redo: silence first, chords last.
SESSION='silence|30|Room silence. Do not touch the phone or the guitar.
e2|10|Low E, open, normal strength. Let it ring.
a2|10|A string, open, normal strength. Let it ring.
d3|10|D string, open, normal strength. Let it ring.
g3|10|G string, open, normal strength. Let it ring.
b3|10|B string, open, normal strength. Let it ring.
e4|10|High E, open, normal strength. Let it ring.
e2_repeat|12|Low E, five plucks, about two seconds apart.
e2_soft|10|Low E, plucked softly. Let it ring.
e2_hand|10|Pick the phone up, hold it as you would to tune. Low E, let it ring.
fifth_a2_e3|10|Phone back down. A string and D string at fret 2 (E3) together: a fifth. Let it ring.
fourth_e2_a2|10|Low E and A string together: a fourth. Let it ring.
strum_open|10|Strum all the open strings once. Let it ring.
strum_slow|10|Strum all the open strings slowly, low to high, once. Let it ring.'

session() {
    local name seconds prompt answer failed=0
    echo "Takes go to $DIR"
    echo "Tune to a trusted reference first. Phone on something soft, about 30 cm from the soundhole,"
    echo "its bottom edge toward the guitar. Keep the room quiet. Start playing at the ● line."
    while IFS='|' read -r name seconds prompt <&3; do
        while true; do
            echo
            echo "── $name ($seconds s): $prompt"
            read -r -p "   Enter records, s skips, q quits: " answer
            case "$answer" in
                q) return "$failed" ;;
                s) break ;;
            esac
            if take "$name" "$seconds"; then
                read -r -p "   Enter keeps it and moves on, r records it again: " answer
                [[ "$answer" == r ]] || break
            else
                failed=$((failed + 1))
                echo "   Rejected. Check the phone, then try it again."
            fi
        done
    done 3<<<"$SESSION"
    echo
    echo "✓ session finished: $DIR"
    return 0
}

case "${1:-}" in
    take) take "${2:-}" "${3:-10}" ;;
    session) session ;;
    *) echo "usage: tools/capture-android.sh take <name> [seconds] | session" >&2; exit 1 ;;
esac

#!/usr/bin/env bash
# The one way this repo runs patrol_cli (docs/adr/0026), so its two traps are handled once:
#   - left alone, patrol_cli writes shell-completion setup into the user's ~/.bashrc or ~/.zshrc on
#     its first run; PATROL_NO_COMPLETION is its own switch for that;
#   - with more than one device attached (a desktop Chrome counts), it asks which one and, with no
#     terminal to answer, repeats the question forever; so the device is always named here.
#
# The CLI is a dev dependency of apps/diapason, pinned by pubspec.lock (doctor checks it), so there
# is no global install to drift.
#
# Usage: tools/patrol.sh <patrol command> [args…]   e.g. tools/patrol.sh test --target …
#        ANDROID_SERIAL picks the device; without it, the only adb device attached.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../apps/diapason"

export PATROL_NO_COMPLETION=1
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

command="${1:?usage: tools/patrol.sh <patrol command> [args…]}"
shift

device=()
if [[ "$command" == test || "$command" == develop ]]; then
    serial="${ANDROID_SERIAL:-}"
    if [[ -z "$serial" ]]; then
        mapfile -t serials < <(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')
        if [[ ${#serials[@]} -ne 1 ]]; then
            echo "✗ ${#serials[@]} adb devices attached: set ANDROID_SERIAL to choose one" >&2
            exit 1
        fi
        serial="${serials[0]}"
    fi
    device=(--device "$serial")
fi

exec dart run patrol_cli:main "$command" "${device[@]}" "$@" </dev/null

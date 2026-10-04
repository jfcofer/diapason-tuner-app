#!/usr/bin/env bash
# Golden tests. Goldens are pinned to the CI runner image: font rasterisation differs between
# machines, so a golden generated locally will fail elsewhere (docs/TESTING.md §3, docs/adr/0016).
#
#   tools/goldens.sh check    verify goldens
#   tools/goldens.sh update   regenerate them
#
# Never run `flutter test --update-goldens` directly on your own machine.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

mode="${1:-check}"

mapfile -t golden_tests < <(grep -rl "matchesGoldenFile" packages/*/test 2>/dev/null)
if [[ ${#golden_tests[@]} -eq 0 ]]; then
    echo "No golden tests yet - core_ui ships no widgets until milestone M3 (docs/ROADMAP.md)."
    echo "This is a pass, not a skip: there is nothing to regress."
    exit 0
fi

case "$mode" in
    check)  ( cd packages/core_ui && flutter test ) ;;
    update)
        echo "Goldens must be regenerated in the pinned CI environment, not on this machine."
        echo "The mechanism is decided with the first golden test in M3 (docs/adr/0016)."
        exit 1
        ;;
    *) echo "usage: tools/goldens.sh [check|update]" >&2; exit 1 ;;
esac

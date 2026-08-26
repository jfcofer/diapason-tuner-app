#!/usr/bin/env bash
# Run Dart/Flutter tests across the workspace, or for one package.
#
#   tools/test-dart.sh              every package that has tests
#   tools/test-dart.sh core_domain  just that one
#
# core_domain is pure Dart with no Flutter dependency (AGENTS.md §5), so it runs under `dart test`.
# Everything else needs `flutter test`. Picking the right runner per package is this script's whole
# reason to exist.

set -uo pipefail
shopt -s globstar

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

filter="${1:-}"
failed=0
ran=0

# Workspace members, read from the root pubspec so this cannot drift from the actual workspace.
members="$(sed -n '/^workspace:/,/^[a-z_]*:/p' pubspec.yaml | grep -oP '^\s+- \K.+' || true)"

for dir in $members; do
    package="$(basename "$dir")"
    [[ -n "$filter" && "$package" != "$filter" ]] && continue
    # A test/ directory with no *_test.dart in it is not a failure - the app shell had one
    # before it had tests. `flutter test` treats it as an error, so skip it here instead.
    [[ -d "$dir/test" ]] || continue
    compgen -G "$dir/test/**/*_test.dart" >/dev/null 2>&1 ||
        compgen -G "$dir/test/*_test.dart" >/dev/null 2>&1 || continue

    # A package that depends on the Flutter SDK must use `flutter test`; `dart test` cannot load it.
    if grep -qE '^\s+(sdk: flutter|flutter:)' "$dir/pubspec.yaml" 2>/dev/null; then
        runner=(flutter test)
    else
        runner=(dart test)
    fi

    echo "── $package (${runner[0]})"
    ran=$((ran + 1))
    ( cd "$dir" && "${runner[@]}" ) || failed=$((failed + 1))
done

if [[ -n "$filter" && $ran -eq 0 ]]; then
    echo "No workspace package named '$filter' with a test/ directory." >&2
    exit 1
fi

[[ $failed -gt 0 ]] && { echo "✗ $failed package(s) failed"; exit 1; }
echo "✓ $ran package(s) passed"

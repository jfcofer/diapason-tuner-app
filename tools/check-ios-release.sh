#!/usr/bin/env bash
# Verify a built iOS archive - the iOS counterpart of check-android-release.sh.
#
#   tools/check-ios-release.sh [path-to-.app]
#
# Asserts on what would ship, never on the source tree. A file that exists in ios/Runner/ but is
# not in the target's resources is not in the app; that exact bug is why this script exists.
#
#   - PrivacyInfo.xcprivacy is inside the .app (App Store rejects uploads without it)
#   - NSMicrophoneUsageDescription is set (the tuner crashes on first mic access without it)
#   - MinimumOSVersion matches IOS_DEPLOYMENT_TARGET in tools/versions.env
#   - the Rust engine is linked, not just compiled: FRB's entry symbol is exported
#
# Runs on macOS only (plutil, nm), so it sticks to bash 3.2 and BSD tools: no grep -P, no arrays.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
# shellcheck disable=SC1091
source tools/versions.env

FAILURES=0
fail() { printf '  ✗ %s\n' "$1"; [ -n "${2:-}" ] && printf '      %s\n' "$2"; FAILURES=$((FAILURES + 1)); }
ok()   { printf '  ✓ %-40s %s\n' "$1" "${2:-}"; }

app="${1:-}"
if [ -z "$app" ]; then
    # An archive from build-ios, else the unsigned device build from build-ios-unsigned.
    for candidate in apps/diapason/build/ios/archive/Runner.xcarchive/Products/Applications/*.app \
                     apps/diapason/build/ios/iphoneos/*.app; do
        [ -d "$candidate" ] && { app="$candidate"; break; }
    done
fi
if [ -z "$app" ] || [ ! -d "$app" ]; then
    printf '✗ no built .app found\n  Build one first: just build-ios-unsigned prod\n\n'
    exit 1
fi

printf '\niOS release checks %s\n' "$app"

if [ -f "$app/PrivacyInfo.xcprivacy" ]; then
    ok "privacy manifest bundled"
else
    fail "PrivacyInfo.xcprivacy is not in the app" \
         "It must be in the Runner resources build phase. Run: just ios-project"
fi

plist="$app/Info.plist"
if plutil -extract NSMicrophoneUsageDescription raw -o - "$plist" >/dev/null 2>&1; then
    ok "NSMicrophoneUsageDescription set"
else
    fail "NSMicrophoneUsageDescription missing from Info.plist"
fi

min_os="$(plutil -extract MinimumOSVersion raw -o - "$plist" 2>/dev/null)"
if [ "$min_os" = "$IOS_DEPLOYMENT_TARGET" ]; then
    ok "MinimumOSVersion" "$min_os"
else
    fail "MinimumOSVersion is ${min_os:-unset}, pinned $IOS_DEPLOYMENT_TARGET"
fi

# Dart finds the engine by symbol at runtime. If the linker dropped it, the build is green and the
# app dies on launch - the "compiling is not linking" trap from T-001a.
linked=""
for binary in "$app/$(plutil -extract CFBundleExecutable raw -o - "$plist" 2>/dev/null)" \
              "$app"/Frameworks/*.framework/*; do
    [ -f "$binary" ] || continue
    if nm -gU "$binary" 2>/dev/null | grep -q 'frb_get_rust_content_hash$'; then
        linked="${binary#"$app"/}"
        break
    fi
done
if [ -n "$linked" ]; then
    ok "Rust engine linked" "$linked"
else
    fail "no binary in the app exports frb_get_rust_content_hash" \
         "The Rust library was compiled but not linked into the app."
fi

printf '\n'
if [ "$FAILURES" -gt 0 ]; then
    printf '✗ %d release blocker(s)\n\n' "$FAILURES"; exit 1
fi
printf '✓ iOS release checks pass\n\n'

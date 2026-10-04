#!/usr/bin/env bash
# Verify a release Android artifact against the 2026 store requirements.
#
#   tools/check-android-release.sh [path-to-aab-or-apk]
#
# Checks, all of which are release blockers (docs/PLATFORM_AUDIO.md §2, docs/CI_RELEASE.md §5):
#   - targetSdk / compileSdk match the pins in tools/versions.env
#   - minSdk matches the pin
#   - EVERY native .so is 16 KB page-aligned, per ABI
#   - the artifact is under the size budget
#
# The 16 KB check is the one that matters most: NDK r28+ aligns by default, so this is expected to
# pass - and that is exactly why it must be verified rather than assumed. A regression here is a
# crash on affected devices, not a warning.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
# shellcheck disable=SC1091
source tools/versions.env

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; BOLD=$'\033[1m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; BOLD=''; OFF=''
fi

FAILURES=0
fail() { printf '  %s✗%s %s\n' "$RED" "$OFF" "$1"; [[ -n "${2:-}" ]] && printf '      %s%s%s\n' "$DIM" "$2" "$OFF"; FAILURES=$((FAILURES+1)); }
ok()   { printf '  %s✓%s %-46s %s%s%s\n' "$GREEN" "$OFF" "$1" "$DIM" "${2:-}" "$OFF"; }

SIZE_BUDGET_MB="${SIZE_BUDGET_MB:-60}"

artifact="${1:-}"
if [[ -z "$artifact" ]]; then
    artifact="$(find apps/diapason/build/app/outputs -name '*prod-release*.aab' -o -name '*prod-release*.apk' 2>/dev/null | head -1)"
fi
if [[ -z "$artifact" || ! -f "$artifact" ]]; then
    printf '%s✗ no release artifact found%s\n' "$RED" "$OFF"
    printf '  Build one first: just build-android prod\n'
    printf '  Or pass a path:  tools/check-android-release.sh <path-to-aab>\n\n'
    exit 1
fi

if [[ "$artifact" == *debug* ]]; then
    printf '\n%s✗ %s is a debug artifact%s\n' "$RED" "$artifact" "$OFF"
    printf '  These are release checks. A debug build ships Vulkan validation layers and debug\n'
    printf '  symbols, so its size and contents say nothing about what users would download.\n'
    printf '  Build a release artifact first: just build-android prod\n\n'
    exit 1
fi

printf '\n%sAndroid release checks%s %s%s%s\n' "$BOLD" "$OFF" "$DIM" "$artifact" "$OFF"

# ── SDK levels ───────────────────────────────────────────────────────────────
# Newest build-tools wins. An unmatched glob prints itself, hence the -x test.
aapt2="$(printf '%s\n' "${ANDROID_HOME:-$HOME/Android/Sdk}"/build-tools/*/aapt2 | sort -V | tail -1)"
if [[ ! -x "$aapt2" ]]; then
    fail "aapt2 not found" "install Android build-tools"
elif [[ "$artifact" == *.apk ]]; then
    badging="$("$aapt2" dump badging "$artifact" 2>/dev/null)"
    target="$(grep -oP "targetSdkVersion:'\K[0-9]+" <<<"$badging")"
    # Note the capital S: aapt2 prints minSdkVersion / targetSdkVersion.
    minsdk="$(grep -oP "minSdkVersion:'\K[0-9]+" <<<"$badging")"
    if [[ "$target" == "$ANDROID_TARGET_SDK" ]]; then
        ok "targetSdk" "$target"
    else
        fail "targetSdk is $target, pinned $ANDROID_TARGET_SDK" \
             "Google Play requires API $ANDROID_TARGET_SDK for submissions from 2026-08-31."
    fi
    if [[ "$minsdk" == "$ANDROID_MIN_SDK" ]]; then
        ok "minSdk" "$minsdk"
    else
        fail "minSdk is $minsdk, pinned $ANDROID_MIN_SDK"
    fi
else
    # An AAB carries its manifest in protobuf; read the pin from Gradle's own source of truth.
    ok "targetSdk (from versions.env)" "$ANDROID_TARGET_SDK"
fi

# ── 16 KB page alignment, per ABI ────────────────────────────────────────────
readelf=""
for candidate in "${ANDROID_HOME:-$HOME/Android/Sdk}"/ndk/"$ANDROID_NDK_VERSION"/toolchains/llvm/prebuilt/*/bin/llvm-readelf; do
    [[ -x "$candidate" ]] && { readelf="$candidate"; break; }
done
[[ -z "$readelf" ]] && readelf="$(command -v llvm-readelf || command -v readelf)"

work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
unzip -o -q "$artifact" -d "$work" 2>/dev/null

mapfile -t sos < <(find "$work" -name '*.so' 2>/dev/null)
if [[ ${#sos[@]} -eq 0 ]]; then
    fail "no .so found in the artifact" "an app with a Rust engine must ship native libraries"
else
    for so in "${sos[@]}"; do
        rel="${so#"$work"/}"
        abi="$(basename "$(dirname "$so")")"
        aligns="$("$readelf" -l "$so" 2>/dev/null | awk '/LOAD/{print $NF}' | sort -u)"

        # 16 KB pages are a 64-bit ABI requirement. Reporting a 4 KB-aligned 32-bit library as
        # "16 KB aligned" would be a lie in a release gate, so say what is actually true.
        if [[ "$abi" != "arm64-v8a" && "$abi" != "x86_64" ]]; then
            ok "$abi/$(basename "$so")" "$aligns (32-bit ABI, 16 KB not required)"
            continue
        fi

        worst=0
        for a in $aligns; do
            v=$(printf '%d' "$a")
            (( worst == 0 || v < worst )) && worst=$v
        done
        if (( worst < 16384 )); then
            fail "$rel: smallest LOAD align $(printf '0x%x' "$worst") (< 0x4000)" \
                 "16 KB alignment is mandatory on 64-bit ABIs. NDK r28+ does this by default - check ndkVersion."
        else
            ok "16 KB aligned: $abi/$(basename "$so")" "$aligns"
        fi
    done
fi

# ── Size budget ──────────────────────────────────────────────────────────────
size_mb=$(( $(stat -c%s "$artifact") / 1024 / 1024 ))
if [[ $size_mb -gt $SIZE_BUDGET_MB ]]; then
    fail "artifact is ${size_mb} MB (budget ${SIZE_BUDGET_MB} MB)" \
         "Raise SIZE_BUDGET_MB deliberately, or find out what grew."
else
    ok "size" "${size_mb} MB / ${SIZE_BUDGET_MB} MB"
fi

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ %d release blocker(s)%s\n\n' "$RED" "$FAILURES" "$OFF"; exit 1
fi
printf '%s✓ release checks pass%s\n\n' "$GREEN" "$OFF"

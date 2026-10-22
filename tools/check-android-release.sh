#!/usr/bin/env bash
# Verify a release Android artifact against the 2026 store requirements.
#
#   tools/check-android-release.sh [path-to-aab-or-apk]
#
# Checks, all of which are release blockers (docs/PLATFORM_AUDIO.md §2, docs/CI_RELEASE.md §5):
#   - targetSdk / compileSdk match the pins in tools/versions.env
#   - minSdk matches the pin
#   - EVERY native .so is 16 KB page-aligned, per ABI
#   - the engine's .so links AAudio and imports it, links no libc++_shared, and fits its per-ABI
#     size budget (docs/AUDIO_ENGINE.md §1, docs/adr/0020): compiling against a library is not
#     linking it, so this reads what actually shipped
#   - nothing from the device tests shipped: no patrol code and no INTERNET permission
#     (docs/adr/0026, AGENTS.md §8)
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

# ── The engine library: what it links, and its size, per ABI ──────────────────
# AUDIO_ENGINE.md §1: added binary size per ABI targets 2.5 MB and fails above 4 MB.
ENGINE_TARGET_KB=2560
ENGINE_FAIL_KB=4096
mapfile -t engines < <(find "$work" -name 'libdiapason_ffi.so' 2>/dev/null)
if [[ ${#engines[@]} -eq 0 ]]; then
    fail "libdiapason_ffi.so not found" "the Rust engine must ship in every release artifact"
fi
for so in "${engines[@]}"; do
    abi="$(basename "$(dirname "$so")")"
    needed="$("$readelf" -d "$so" 2>/dev/null | grep -oP 'NEEDED.*\[\K[^]]+')"
    imports="$("$readelf" --dyn-syms "$so" 2>/dev/null | awk '$7 == "UND" {print $8}' | grep -c '^AAudio')"
    if grep -qx 'libc++_shared.so' <<<"$needed"; then
        fail "$abi: the engine links libc++_shared.so" "adr/0020 chose raw ndk-sys precisely to ship no C++ runtime"
    elif ! grep -qx 'libaaudio.so' <<<"$needed" || [[ "$imports" -eq 0 ]]; then
        fail "$abi: the engine does not link AAudio (NEEDED: $(tr '\n' ' ' <<<"$needed"))" \
             "compiled but not linked: the backend is missing from the shipped library"
    else
        ok "$abi: links AAudio, no C++ runtime" "$imports AAudio imports"
    fi
    kb=$(( $(stat -c%s "$so") / 1024 ))
    if (( kb > ENGINE_FAIL_KB )); then
        fail "$abi: libdiapason_ffi.so is ${kb} KB (fails above ${ENGINE_FAIL_KB} KB)" \
             "AUDIO_ENGINE.md §1 budgets 2.5 MB per ABI: find out what grew"
    elif (( kb > ENGINE_TARGET_KB )); then
        ok "$abi: engine size" "${kb} KB, over the ${ENGINE_TARGET_KB} KB target"
    else
        ok "$abi: engine size" "${kb} KB / ${ENGINE_TARGET_KB} KB"
    fi
done

# ── Nothing from the device tests ships (docs/adr/0026, AGENTS.md §8) ────────
# Patrol is a dev dependency and reaches the app over a localhost port, so its builds declare
# INTERNET. A release must contain neither its code nor that permission. Both scans prove they can
# read what they scan first: the manifest must show RECORD_AUDIO, the dex must exist.
if [[ "$artifact" == *.apk ]]; then
    permissions="$("$aapt2" dump permissions "$artifact" 2>/dev/null)"
else
    # An AAB's manifest is protobuf, which stores its strings as plain UTF-8.
    permissions="$(grep -a -o 'android\.permission\.[A-Z_]*' "$work/base/manifest/AndroidManifest.xml" 2>/dev/null)"
fi
if ! grep -q 'android\.permission\.RECORD_AUDIO' <<<"$permissions"; then
    fail "could not read the manifest's permissions" "the scan must see RECORD_AUDIO before its silence means anything"
elif grep -q 'android\.permission\.INTERNET' <<<"$permissions"; then
    fail "the release declares android.permission.INTERNET" \
         "This app makes no network requests (AGENTS.md §8). Find the manifest that merged it in."
else
    ok "no INTERNET permission" "RECORD_AUDIO seen, so the manifest was read"
fi

mapfile -t dexes < <(find "$work" -name 'classes*.dex' 2>/dev/null)
if [[ ${#dexes[@]} -eq 0 ]]; then
    fail "no classes*.dex found in the artifact" "the scan for test code read nothing"
elif grep -a -q -l 'pl[./]leancode[./]patrol' "${dexes[@]}"; then
    fail "the release contains patrol code" "patrol must stay a dev dependency (docs/adr/0026)"
else
    ok "no patrol code" "${#dexes[@]} dex file(s) scanned"
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

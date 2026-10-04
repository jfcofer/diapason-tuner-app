#!/usr/bin/env bash
# Verify every pinned version in tools/versions.env before you waste an hour.
#
# This is the most valuable script in the repo. Its job is to fail LOUDLY, with the fix command in
# the error text, on the handful of mismatches that otherwise surface as an incomprehensible
# Gradle, Xcode or linker error forty minutes into a build.
#
#   tools/doctor.sh                  every section - what a developer machine needs
#   tools/doctor.sh flutter frb      only these sections - what one CI job installed
#
# Sections: flutter java rust rust-tools frb android dev. Each CI job runs doctor on exactly the
# toolchain it set up, so every job proves its own versions match tools/versions.env.
#
# Exit 0 = every check passed. Exit 1 = at least one hard failure (or an unknown section).

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
# shellcheck disable=SC1091
source tools/versions.env

SECTIONS=(flutter java rust rust-tools frb android dev)
for arg in "$@"; do
    [[ " ${SECTIONS[*]} " == *" $arg "* ]] || { echo "doctor: unknown section '$arg' (have: ${SECTIONS[*]})" >&2; exit 1; }
done
REQUESTED=("$@")
# No arguments means every section.
want() { [[ ${#REQUESTED[@]} -eq 0 || " ${REQUESTED[*]} " == *" $1 "* ]]; }

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; DIM=$'\033[2m'; BOLD=$'\033[1m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; DIM=''; BOLD=''; OFF=''
fi

FAILURES=0
WARNINGS=0

pass() { printf '  %s✓%s %-34s %s%s%s\n' "$GREEN" "$OFF" "$1" "$DIM" "${2:-}" "$OFF"; }
warn() { printf '  %s!%s %-34s %s\n' "$YELLOW" "$OFF" "$1" "$2"; WARNINGS=$((WARNINGS + 1)); }
fail() {
    printf '  %s✗%s %-34s %s%s%s\n' "$RED" "$OFF" "$1" "$BOLD" "$2" "$OFF"
    [[ -n "${3:-}" ]] && printf '      %sfix:%s %s\n' "$DIM" "$OFF" "$3"
    FAILURES=$((FAILURES + 1))
}
section() { printf '\n%s%s%s\n' "$BOLD" "$1" "$OFF"; }
# Tools print their versions in different shapes; take the first semver from either stream.
semver_of() { "$@" 2>&1 | grep -oP '[0-9]+\.[0-9]+\.[0-9]+' | head -1; }

# ── Flutter / Dart ────────────────────────────────────────────────────────────
if want flutter; then
section "Flutter / Dart"

if ! command -v flutter >/dev/null 2>&1; then
    fail "flutter" "not on PATH" "install Flutter $FLUTTER_VERSION, or: fvm install && fvm use"
else
    actual="$(flutter --version 2>/dev/null | head -1 | awk '{print $2}')"
    if [[ "$actual" == "$FLUTTER_VERSION" ]]; then
        pass "flutter" "$actual"
    else
        fail "flutter" "have $actual, need $FLUTTER_VERSION" \
             "fvm install $FLUTTER_VERSION && fvm use $FLUTTER_VERSION"
    fi
fi

# .fvmrc must agree with versions.env, or the two pins have silently diverged.
if [[ -f .fvmrc ]]; then
    fvmrc="$(grep -oP '"flutter"\s*:\s*"\K[^"]+' .fvmrc 2>/dev/null)"
    if [[ "$fvmrc" == "$FLUTTER_VERSION" ]]; then
        pass ".fvmrc agrees with versions.env" "$fvmrc"
    else
        fail ".fvmrc" "says $fvmrc, versions.env says $FLUTTER_VERSION" \
             "make them match - versions.env is the source of truth"
    fi
else
    fail ".fvmrc" "missing" "echo '{\"flutter\": \"$FLUTTER_VERSION\"}' > .fvmrc"
fi

if command -v dart >/dev/null 2>&1; then
    actual="$(dart --version 2>&1 | grep -oP 'version: \K[0-9]+\.[0-9]+\.[0-9]+')"
    if [[ "$actual" == "$DART_VERSION" ]]; then
        pass "dart" "$actual"
    else
        fail "dart" "have $actual, need $DART_VERSION" "comes with Flutter $FLUTTER_VERSION"
    fi
else
    fail "dart" "not on PATH" "install Flutter $FLUTTER_VERSION"
fi

# Melos runs as `dart run melos` from the workspace dev_dependencies, so the lockfile is the pin -
# no global install to drift.
melos_locked="$(awk '/^  melos:$/{f=1} f&&/version:/{gsub(/"/,"",$2); print $2; exit}' pubspec.lock 2>/dev/null)"
if [[ "$melos_locked" == "$MELOS_VERSION" ]]; then
    pass "melos (pubspec.lock)" "$melos_locked"
else
    fail "melos (pubspec.lock)" "locked ${melos_locked:-nothing}, pinned $MELOS_VERSION" \
         "set melos to $MELOS_VERSION in pubspec.yaml, then: dart pub get"
fi
fi

# ── Java ──────────────────────────────────────────────────────────────────────
# The JDK that matters is the one *Flutter* hands to Gradle, which is not necessarily the one on
# PATH. Flutter prefers, in order: its own jdk-dir config, then Android Studio's bundled JBR, then
# JAVA_HOME. Android Studio currently bundles JBR 25, which is exactly the version that breaks.
if want java; then
section "Java (the JDK Gradle will actually use)"

flutter_jdk=""
if [[ -f "$HOME/.config/flutter/settings" ]] && command -v python3 >/dev/null 2>&1; then
    flutter_jdk="$(python3 -c '
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("jdk-dir", ""))
except Exception:
    print("")
' "$HOME/.config/flutter/settings" 2>/dev/null)"
fi

java_bin=""
if [[ -n "$flutter_jdk" && -x "$flutter_jdk/bin/java" ]]; then
    java_bin="$flutter_jdk/bin/java"
    java_src="flutter config --jdk-dir"
elif [[ -n "${JAVA_HOME:-}" && -x "${JAVA_HOME}/bin/java" ]]; then
    java_bin="$JAVA_HOME/bin/java"
    java_src="JAVA_HOME"
elif command -v java >/dev/null 2>&1; then
    java_bin="$(command -v java)"
    java_src="PATH"
fi

jdk_fix="flutter config --jdk-dir=\"\$HOME/.sdkman/candidates/java/$JAVA_SDKMAN_ID\""
if [[ -z "$java_bin" ]]; then
    fail "java" "no JDK found" "sdk install java $JAVA_SDKMAN_ID && $jdk_fix"
else
    major="$("$java_bin" -version 2>&1 | head -1 | grep -oP '"\K[0-9]+')"
    if [[ "$major" == "$JAVA_MAJOR" ]]; then
        pass "java $major (via $java_src)" "$java_bin"
    elif [[ "$major" -ge 25 ]] 2>/dev/null; then
        fail "java" "JDK $major via $java_src - Flutter Android builds FAIL on JDK 25+" \
             "flutter/flutter#187223. sdk install java $JAVA_SDKMAN_ID && $jdk_fix"
    else
        fail "java" "JDK $major via $java_src, need $JAVA_MAJOR" \
             "sdk install java $JAVA_SDKMAN_ID && $jdk_fix"
    fi
fi
fi

# ── Rust ──────────────────────────────────────────────────────────────────────
if want rust; then
section "Rust"

if ! command -v rustc >/dev/null 2>&1; then
    fail "rustc" "not on PATH" "https://rustup.rs"
else
    # rust-toolchain.toml selects the compiler, so check it agrees with the pin, then check rustc.
    channel="$(grep -oP '^\s*channel\s*=\s*"\K[^"]+' rust-toolchain.toml 2>/dev/null)"
    if [[ "$channel" == "$RUST_VERSION" ]]; then
        pass "rust-toolchain.toml channel" "$channel"
    else
        fail "rust-toolchain.toml channel" "says ${channel:-nothing}, versions.env says $RUST_VERSION" \
             "make them match - versions.env is the source of truth"
    fi
    actual="$(rustc --version | awk '{print $2}')"
    if [[ "$actual" == "$RUST_VERSION" ]]; then
        pass "rustc" "$actual"
    else
        fail "rustc" "have $actual, need $RUST_VERSION" "rustup toolchain install"
    fi

    installed="$(rustup target list --installed 2>/dev/null)"
    missing=()
    while read -r target; do
        [[ -z "$target" ]] && continue
        grep -qx "$target" <<<"$installed" || missing+=("$target")
    done < <(grep -oP '^\s*"\K[a-z0-9_]+-[a-z0-9_-]+(?=")' rust-toolchain.toml 2>/dev/null)

    if [[ ${#missing[@]} -eq 0 ]]; then
        pass "cross-compile targets" "$(grep -c . <<<"$installed") installed"
    else
        fail "cross-compile targets" "missing: ${missing[*]}" "rustup target add ${missing[*]}"
    fi
fi
fi

# The test and supply-chain tools. Not cargo-ndk: cargokit drives the NDK linker itself.
if want rust-tools; then
section "Rust tools"
for tool in cargo-nextest cargo-deny; do
    if command -v "$tool" >/dev/null 2>&1; then
        pass "$tool" "$(semver_of cargo "${tool#cargo-}" --version)"
    else
        fail "$tool" "not installed" "cargo install $tool --locked"
    fi
done
fi

# ── flutter_rust_bridge: the three-way version check ──────────────────────────
# The codegen binary, the Rust crate and the Dart package must be the same version. Any two of
# them agreeing is not enough - that is precisely how this fails in practice.
if want frb; then
section "flutter_rust_bridge (three-way version match)"

frb_codegen=""
if command -v flutter_rust_bridge_codegen >/dev/null 2>&1; then
    frb_codegen="$(flutter_rust_bridge_codegen --version 2>/dev/null | grep -oP '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
fi
# Tolerate the caret/tilde/equals prefixes Cargo allows: "=2.13.0", "^2.13.0", "2.13.0", and
# the { version = "..." } table form.
frb_rust="$(grep -oP '^\s*flutter_rust_bridge\s*=\s*(\{[^}]*version\s*=\s*)?"[=^~]?\K[0-9]+\.[0-9]+\.[0-9]+' Cargo.toml 2>/dev/null | head -1)"
frb_dart="$(grep -oP '^\s*flutter_rust_bridge:\s*\^?\K[0-9]+\.[0-9]+\.[0-9]+' packages/audio_engine/pubspec.yaml 2>/dev/null | head -1)"

if [[ -z "$frb_codegen" ]]; then
    fail "frb codegen binary" "not installed" \
         "cargo install flutter_rust_bridge_codegen --version $FRB_VERSION --locked"
elif [[ "$frb_codegen" != "$FRB_VERSION" ]]; then
    fail "frb codegen binary" "have $frb_codegen, pinned $FRB_VERSION" \
         "cargo install flutter_rust_bridge_codegen --version $FRB_VERSION --locked --force"
else
    pass "frb codegen binary" "$frb_codegen"
fi

# Before the scaffold exists these files are absent. Warn rather than fail, so doctor is usable
# during T-001 itself - but never let a file that EXISTS disagree with the pin.
check_frb_pin() {
    local label="$1" have="$2" file="$3"
    if [[ -z "$have" ]]; then
        if [[ -f "$file" ]]; then
            fail "$label" "no flutter_rust_bridge dependency in $file" "add flutter_rust_bridge $FRB_VERSION"
        else
            warn "$label" "$file not created yet (T-001 scaffold)"
        fi
    elif [[ "$have" != "$FRB_VERSION" ]]; then
        fail "$label" "$file says $have, pinned $FRB_VERSION" "set it to $FRB_VERSION"
    else
        pass "$label" "$have"
    fi
}
check_frb_pin "frb Rust crate"   "$frb_rust" "Cargo.toml"
check_frb_pin "frb Dart package" "$frb_dart" "packages/audio_engine/pubspec.yaml"
fi

# ── Android SDK ───────────────────────────────────────────────────────────────
if want android; then
section "Android SDK"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
if [[ ! -d "$sdk" ]]; then
    fail "Android SDK" "not found at $sdk" "set ANDROID_HOME, or install via Android Studio"
else
    pass "Android SDK" "$sdk"

    if [[ -d "$sdk/platforms/android-$ANDROID_COMPILE_SDK" ]]; then
        pass "platform android-$ANDROID_COMPILE_SDK" "present"
    else
        fail "platform android-$ANDROID_COMPILE_SDK" "not installed" \
             "sdkmanager \"platforms;android-$ANDROID_COMPILE_SDK\""
    fi

    if [[ -d "$sdk/ndk/$ANDROID_NDK_VERSION" ]]; then
        pass "NDK $ANDROID_NDK_VERSION" "16 KB page alignment by default"
    else
        fail "NDK $ANDROID_NDK_VERSION" "not installed (have: $(ls "$sdk/ndk" 2>/dev/null | tr '\n' ' '))" \
             "sdkmanager \"ndk;$ANDROID_NDK_VERSION\""
    fi
fi

fi

# ── Developer machine only ────────────────────────────────────────────────────
if want dev; then
section "Developer machine"

if command -v lefthook >/dev/null 2>&1; then
    pass "lefthook" "$(semver_of lefthook --version)"
else
    fail "lefthook" "not on PATH" "see docs/DEVELOPMENT.md §2"
fi
fi

# ── Verdict ───────────────────────────────────────────────────────────────────
printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ %d check(s) failed%s' "$RED" "$FAILURES" "$OFF"
    [[ $WARNINGS -gt 0 ]] && printf ', %d warning(s)' "$WARNINGS"
    printf '\n  Fix the above before building. Versions are pinned in tools/versions.env,\n'
    printf '  explained in docs/DEVELOPMENT.md §1, and changing one is its own ADR.\n\n'
    exit 1
fi

printf '%s✓ toolchain matches tools/versions.env%s' "$GREEN" "$OFF"
[[ $WARNINGS -gt 0 ]] && printf ' (%d warning(s) - expected while T-001 is in progress)' "$WARNINGS"
printf '\n\n'
exit 0

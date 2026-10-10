#!/usr/bin/env bash
# Prove that tools/doctor.sh actually fails.
#
# A doctor that has only ever been run on a healthy machine is not a check - it is a green light
# with no bulb behind it. This deliberately breaks each pin in a scratch copy of the repo and
# asserts doctor catches it, then asserts doctor is green when nothing is broken.
#
# Runs entirely in a temp directory. Never mutates the working tree or any machine-level config.
#
#   tools/doctor-selftest.sh                 every section
#   tools/doctor-selftest.sh flutter frb     only cases for these sections, and a control run of
#                                            `doctor.sh flutter frb` - for a CI job that installed
#                                            only part of the toolchain
#
# Each case runs doctor on its own section only, so "failed, but not about this" cannot be masked
# by an unrelated section that happens to be red on this machine.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
REPO="$PWD"
REQUESTED=("$@")
want() { [[ ${#REQUESTED[@]} -eq 0 || " ${REQUESTED[*]} " == *" $1 "* ]]; }

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; BOLD=$'\033[1m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; BOLD=''; OFF=''
fi

WORK="$(mktemp -d)"
trap 'chmod -R u+w "$WORK" 2>/dev/null; rm -rf -- "$WORK"' EXIT

FAILURES=0

# Assert that `doctor <section>`, run in a scratch repo mutated by $3, exits non-zero and its
# output matches $4.
expect_failure() {
    local section="$1" name="$2" mutate="$3" pattern="$4" env_prefix="${5:-}"
    want "$section" || return 0
    local sandbox="$WORK/$RANDOM$RANDOM"

    mkdir -p "$sandbox/tools"
    cp "$REPO/tools/doctor.sh" "$REPO/tools/jvm.sh" "$REPO/tools/versions.env" "$sandbox/tools/"
    cp "$REPO/.fvmrc" "$REPO/rust-toolchain.toml" "$REPO/pubspec.lock" "$sandbox/"

    ( cd "$sandbox" && eval "$mutate" )

    local out rc
    out="$( cd "$sandbox" && eval "$env_prefix bash tools/doctor.sh $section" 2>&1 )"
    rc=$?

    if [[ $rc -eq 0 ]]; then
        printf '  %s✗%s %-38s %sdoctor passed - it should have failed%s\n' "$RED" "$OFF" "$name" "$BOLD" "$OFF"
        FAILURES=$((FAILURES + 1))
    elif ! grep -qiE "$pattern" <<<"$out"; then
        printf '  %s✗%s %-38s %sfailed, but not about this%s\n' "$RED" "$OFF" "$name" "$BOLD" "$OFF"
        printf '      %sexpected output matching:%s %s\n' "$DIM" "$OFF" "$pattern"
        FAILURES=$((FAILURES + 1))
    else
        printf '  %s✓%s %-38s %scaught%s\n' "$GREEN" "$OFF" "$name" "$DIM" "$OFF"
    fi
}

printf '\n%sdoctor self-test%s\n' "$BOLD" "$OFF"

# 1. The trap that motivated the whole check: Flutter Android builds fail on JDK 25.
#    Point doctor at a real JDK >= 25 and require it to name the actual issue, not just "wrong
#    version" - the error text is the whole value of this check.
JDK25=""
# GitHub's ubuntu runners keep their JDKs as temurin-NN-jdk-amd64 and export JAVA_HOME_25_X64.
for candidate in "${JAVA_HOME_25_X64:-}" /usr/lib/jvm/java-2[5-9]-openjdk /usr/lib/jvm/java-2[5-9]* \
        /usr/lib/jvm/temurin-2[5-9]*; do
    [[ -x "$candidate/bin/java" ]] && { JDK25="$candidate"; break; }
done
if ! want java; then
    :
elif [[ -n "$JDK25" && -x "$JDK25/bin/java" ]]; then
    expect_failure java "JDK 25 named as the known-bad case" \
        'true' \
        'Flutter Android builds FAIL on JDK 25' \
        "HOME=\"$WORK/nohome\" JAVA_HOME=\"$JDK25\""
else
    printf '  %s-%s %-38s %sno JDK 25+ on this machine to test with%s\n' "$DIM" "$OFF" \
        "JDK 25 named as the known-bad case" "$DIM" "$OFF"
fi

# 1b. Any other wrong major is rejected too.
expect_failure java "wrong Java major is rejected" \
    'sed -i "s/^JAVA_MAJOR=.*/JAVA_MAJOR=99/" tools/versions.env' \
    'java.*need 99'

# 2. flutter_rust_bridge codegen binary vs the pin.
expect_failure frb "frb codegen version skew" \
    'sed -i "s/^FRB_VERSION=.*/FRB_VERSION=1.0.0/" tools/versions.env' \
    'frb codegen binary.*1\.0\.0'

# 3. frb Rust crate disagreeing with the pin - the three-way check's second leg.
expect_failure frb "frb Rust crate version skew" \
    'printf "[workspace]\nmembers = []\n\n[workspace.dependencies]\nflutter_rust_bridge = \"2.0.0\"\n" > Cargo.toml' \
    'frb Rust crate.*2\.0\.0'

# 4. .fvmrc silently diverging from versions.env.
expect_failure flutter ".fvmrc diverged from versions.env" \
    'printf "{\n  \"flutter\": \"3.0.0\"\n}\n" > .fvmrc' \
    '\.fvmrc.*3\.0\.0'

# 5. Flutter itself at the wrong version.
expect_failure flutter "wrong Flutter version" \
    'sed -i "s/^FLUTTER_VERSION=.*/FLUTTER_VERSION=1.2.3/" tools/versions.env' \
    'flutter.*need 1\.2\.3'

# 6. A cross-compile target from rust-toolchain.toml missing.
expect_failure rust "missing rustup target" \
    'sed -i "s|\"aarch64-apple-ios\",|\"aarch64-apple-ios\",\n    \"sparc64-unknown-netbsd\",|" rust-toolchain.toml' \
    'cross-compile targets.*sparc64-unknown-netbsd'

# 6b. rust-toolchain.toml floating away from the Rust pin.
expect_failure rust "rust channel diverged from versions.env" \
    'sed -i "s/^channel = .*/channel = \"stable\"/" rust-toolchain.toml' \
    'rust-toolchain\.toml channel.*stable'

# 6c. The active rustc differing from the pin.
expect_failure rust "wrong rustc version" \
    'sed -i "s/^RUST_VERSION=.*/RUST_VERSION=0.0.1/" tools/versions.env' \
    'rustc .*need 0\.0\.1'

# 6d. Repo check tools at the wrong version.
expect_failure repo-tools "wrong actionlint version" \
    'sed -i "s/^ACTIONLINT_VERSION=.*/ACTIONLINT_VERSION=0.0.1/" tools/versions.env' \
    'actionlint.*need 0\.0\.1'
expect_failure repo-tools "wrong xcodeproj gem version" \
    'sed -i "s/^XCODEPROJ_VERSION=.*/XCODEPROJ_VERSION=0.0.1/" tools/versions.env' \
    'xcodeproj gem.*0\.0\.1 not installed'
expect_failure repo-tools "wrong ktfmt version" \
    'sed -i "s/^KTFMT_VERSION=.*/KTFMT_VERSION=0.0.1/" tools/versions.env' \
    'ktfmt.*0\.0\.1 not installed'
# The jar at the pinned version is present, but it is not the one the pin vouches for.
expect_failure repo-tools "ktfmt jar fails its checksum" \
    'sed -i "s/^KTFMT_SHA256=.*/KTFMT_SHA256=0000000000000000000000000000000000000000000000000000000000000000/" tools/versions.env' \
    'ktfmt.*does not match the pinned checksum'

# 7. Missing Android NDK.
expect_failure android "wrong Android NDK" \
    'sed -i "s/^ANDROID_NDK_VERSION=.*/ANDROID_NDK_VERSION=1.2.3/" tools/versions.env' \
    'NDK 1\.2\.3.*not installed'

# 8. melos is pinned by the lockfile, not a global binary - a lockfile that drifts is caught.
expect_failure flutter "melos lockfile drift" \
    'sed -i "s/^MELOS_VERSION=.*/MELOS_VERSION=0.0.1/" tools/versions.env' \
    'melos \(pubspec\.lock\).*pinned 0\.0\.1'

# 9. And the control: unmutated, doctor must be green on this machine.
printf '\n%scontrol%s\n' "$BOLD" "$OFF"
if bash "$REPO/tools/doctor.sh" "${REQUESTED[@]}" >/dev/null 2>&1; then
    printf '  %s✓%s %-38s %sgreen on this machine%s\n' "$GREEN" "$OFF" "unmutated doctor passes" "$DIM" "$OFF"
else
    printf '  %s✗%s %-38s %sdoctor is red - run: just doctor%s\n' "$RED" "$OFF" "unmutated doctor passes" "$BOLD" "$OFF"
    FAILURES=$((FAILURES + 1))
fi

printf '\n'
if [[ $FAILURES -gt 0 ]]; then
    printf '%s✗ doctor self-test: %d failure(s)%s\n\n' "$RED" "$FAILURES" "$OFF"
    exit 1
fi
printf '%s✓ doctor detects every pin it claims to%s\n\n' "$GREEN" "$OFF"
exit 0

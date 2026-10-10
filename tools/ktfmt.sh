#!/usr/bin/env bash
# Format or check the Kotlin we write, with the ktfmt release pinned in tools/versions.env (T-011).
#
#   tools/ktfmt.sh install          download the pinned jar into the user cache and verify its SHA-256
#   tools/ktfmt.sh check            fail if any of our .kt files is not formatted
#   tools/ktfmt.sh format [file…]   format the given files (relative to where you run it), or all ours
#
# Ours means tracked `*.kt` minus generated code (`*.g.kt`, Pigeon's output). Gradle's `*.kts` are
# Flutter's template files and are left as the template writes them (adr/0023).
#
# Exit codes: 0 done, 1 not formatted (check) or ktfmt could not parse a file, 2 ktfmt cannot run
# here (no jar, a jar that fails its checksum, no JDK). One script installs the jar, for
# `just setup` and for CI.

set -uo pipefail
caller="$PWD"
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 2
# shellcheck disable=SC1091
source tools/versions.env
# shellcheck source=tools/jvm.sh
source tools/jvm.sh

jar="$(ktfmt_jar)"
url="https://repo1.maven.org/maven2/com/facebook/ktfmt/$KTFMT_VERSION/ktfmt-$KTFMT_VERSION-with-dependencies.jar"

install() {
    if [[ -f "$jar" && "$(sha256_of "$jar")" == "$KTFMT_SHA256" ]]; then
        echo "ktfmt $KTFMT_VERSION already installed: $jar"
        return 0
    fi
    mkdir -p "$(dirname "$jar")"
    # A unique name beside the target, moved into place only once verified: a failed, tampered or
    # concurrent download never leaves a jar the next run would trust.
    # Script-level, not local: the EXIT trap runs after this function has returned.
    part="$(mktemp "$jar.XXXXXX")" || return 2
    trap 'rm -f "${part:-}"' EXIT
    curl -sSfL --retry 3 -o "$part" "$url" || { echo "✗ download failed: $url" >&2; return 2; }
    local got; got="$(sha256_of "$part")"
    if [[ "$got" != "$KTFMT_SHA256" ]]; then
        echo "✗ ktfmt $KTFMT_VERSION checksum mismatch: got ${got:-nothing}, pinned $KTFMT_SHA256" >&2
        return 2
    fi
    mv "$part" "$jar" || return 2
    echo "ktfmt $KTFMT_VERSION installed: $jar"
}

# Runs ktfmt on the pinned JDK, only if the jar on disk is the one the pin vouches for.
run() {
    [[ -f "$jar" ]] || { echo "✗ ktfmt $KTFMT_VERSION is not installed. Run: just install-ktfmt" >&2; return 2; }
    [[ "$(sha256_of "$jar")" == "$KTFMT_SHA256" ]] || {
        echo "✗ $jar does not match the pinned checksum. Run: rm \"$jar\" && just install-ktfmt" >&2
        return 2
    }
    local java_bin
    java_bin="$(find_jdk | cut -f1)"
    [[ -n "$java_bin" ]] || { echo "✗ no JDK found. Run: just doctor java" >&2; return 2; }
    "$java_bin" -jar "$jar" --kotlinlang-style --quiet "$@"
}

ours() { git ls-files -z '*.kt' ':!*.g.kt'; }

case "${1:-}" in
    install)
        install
        ;;
    check)
        files=()
        while IFS= read -r -d '' f; do files+=("$f"); done < <(ours)
        [[ ${#files[@]} -eq 0 ]] && { echo "no Kotlin files to check"; exit 0; }
        # --dry-run prints each file that would change; --set-exit-if-changed makes that exit 1.
        # A file ktfmt cannot parse also exits 1, but lists nothing, so the message tells them apart.
        out="$(run --dry-run --set-exit-if-changed "${files[@]}")"
        rc=$?
        [[ -n "$out" ]] && printf '%s\n' "$out"
        if [[ $rc -eq 2 ]]; then
            exit 2
        elif [[ $rc -ne 0 && -n "$out" ]]; then
            echo "✗ Kotlin is not formatted. Run: just fix" >&2
            exit 1
        elif [[ $rc -ne 0 ]]; then
            echo "✗ ktfmt failed; see its errors above" >&2
            exit 1
        fi
        ;;
    format)
        shift
        files=()
        for f in "$@"; do
            if [[ "$f" == /* ]]; then files+=("$f"); else files+=("$caller/$f"); fi
        done
        if [[ ${#files[@]} -eq 0 ]]; then
            while IFS= read -r -d '' f; do files+=("$f"); done < <(ours)
        fi
        [[ ${#files[@]} -eq 0 ]] && exit 0
        run "${files[@]}"
        ;;
    *)
        echo "usage: tools/ktfmt.sh install | check | format [file…]" >&2
        exit 2
        ;;
esac

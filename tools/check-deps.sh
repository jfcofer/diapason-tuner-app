#!/usr/bin/env bash
# Enforce the one-way dependency rule in AGENTS.md §5.
#
#     apps/diapason  ->  packages/feature_*  ->  packages/core_ui, core_domain, core_platform
#                                            ->  packages/audio_engine
#                                                    |
#     diapason_ffi -> session -> engine -> dsp          (dsp depends on nothing)
#                             -> audio_io
#
# diapason_ffi lives in packages/audio_engine/rust, where cargokit builds it.
#
# These boundaries are the difference between a codebase that stays navigable and one that turns
# into a graph. They used to be enforced by review, which is to say not enforced. This runs in
# `just verify` and in CI.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if [[ -t 1 ]]; then
    RED=$'\033[31m'; GREEN=$'\033[32m'; DIM=$'\033[2m'; BOLD=$'\033[1m'; OFF=$'\033[0m'
else
    RED=''; GREEN=''; DIM=''; BOLD=''; OFF=''
fi

VIOLATIONS=0

violation() {
    printf '  %s✗%s %s\n' "$RED" "$OFF" "$1"
    printf '      %s%s%s\n' "$DIM" "$2" "$OFF"
    VIOLATIONS=$((VIOLATIONS + 1))
}
ok() { printf '  %s✓%s %s\n' "$GREEN" "$OFF" "$1"; }

# Real Dart imports only: skip comments, doc comments and strings mentioning a package name.
#
# Only directories that exist are searched: grep exits 2 on a missing one, and under pipefail that
# status used to make `imports_of x | grep -q y` false even on a match - so a package without a
# test/ directory could import anything unreported. `|| true` because no matches is not an error.
imports_of() {
    local dirs=() d
    for d in "$1"/lib "$1"/test; do [[ -d "$d" ]] && dirs+=("$d"); done
    [[ ${#dirs[@]} -eq 0 ]] && return 0
    { grep -rhoP "^\s*(import|export)\s+'\K[^']+" "${dirs[@]}" || true; } | sort -u
}

# True if package $1 imports anything matching the regex $2. Captures the whole list first: a
# `grep -q` on a pipe can kill the writer with SIGPIPE, which pipefail reports as failure.
imports_match() {
    local list
    list="$(imports_of "$1")"
    grep -E "$2" >/dev/null <<<"$list"
}

printf '\n%sDependency direction (AGENTS.md §5)%s\n' "$BOLD" "$OFF"

# ── feature_* must never import each other ────────────────────────────────────
for dir in packages/feature_*; do
    [[ -d "$dir" ]] || continue
    self="$(basename "$dir")"
    for other_dir in packages/feature_*; do
        other="$(basename "$other_dir")"
        [[ "$self" == "$other" ]] && continue
        if imports_match "$dir" "^package:$other/"; then
            violation "$self imports $other" \
                "feature packages never import each other. Move the shared code down into core_*."
        fi
    done
done
[[ $VIOLATIONS -eq 0 ]] && ok "no feature_* imports another feature_*"

# ── core_domain is pure Dart ──────────────────────────────────────────────────
before=$VIOLATIONS
while read -r import; do
    case "$import" in
        package:flutter/*|package:flutter_*|dart:io|dart:ui)
            violation "core_domain imports $import" \
                "core_domain is pure Dart: no Flutter, no I/O, no plugins. It must run under \`dart test\`."
            ;;
    esac
done < <(imports_of packages/core_domain)
[[ $VIOLATIONS -eq $before ]] && ok "core_domain is pure Dart"

# ── core_ui owns no state management and no platform access ───────────────────
before=$VIOLATIONS
while read -r import; do
    case "$import" in
        package:riverpod*|package:flutter_riverpod*)
            violation "core_ui imports $import" \
                "core_ui takes its data as arguments so it stays golden-testable. State lives in feature_*."
            ;;
        dart:io)
            violation "core_ui imports $import" \
                "core_ui must not touch the platform directly; inject a core_platform interface."
            ;;
    esac
done < <(imports_of packages/core_ui)
[[ $VIOLATIONS -eq $before ]] && ok "core_ui has no Riverpod and no direct platform access"

# ── nothing may depend on the app shell ───────────────────────────────────────
before=$VIOLATIONS
for dir in packages/*; do
    [[ -d "$dir/lib" ]] || continue
    if imports_match "$dir" "^package:diapason/"; then
        violation "$(basename "$dir") imports the app shell" \
            "A package that depends on apps/diapason is a design error - invert it."
    fi
done
[[ $VIOLATIONS -eq $before ]] && ok "nothing depends on apps/diapason"

# ── core_* must not reach sideways into feature_* ─────────────────────────────
before=$VIOLATIONS
for dir in packages/core_* packages/audio_engine; do
    [[ -d "$dir/lib" ]] || continue
    if imports_match "$dir" "^package:feature_"; then
        violation "$(basename "$dir") imports a feature package" \
            "Dependencies point down, never up."
    fi
done
[[ $VIOLATIONS -eq $before ]] && ok "no core_* or audio_engine imports a feature_*"

# ── Rust: dsp depends on nothing ──────────────────────────────────────────────
printf '\n%sRust crate boundaries%s\n' "$BOLD" "$OFF"

dsp_deps="$(sed -n '/^\[dependencies\]/,/^\[/p' rust/crates/dsp/Cargo.toml 2>/dev/null | grep -oP '^[a-z_]+(?=\s*[=.])' || true)"
if [[ -n "$dsp_deps" ]]; then
    violation "dsp has dependencies: $(tr '\n' ' ' <<<"$dsp_deps")" \
        "rust/crates/dsp is pure computation and depends on nothing - especially not audio_io."
else
    ok "dsp depends on nothing"
fi

if grep -q "diapason_audio_io" rust/crates/dsp/Cargo.toml 2>/dev/null; then
    violation "dsp depends on audio_io" "dsp must stay testable offline with fixture buffers."
fi

# The workspace crates a manifest depends on, in any [dependencies] table (target-specific ones
# included, dev-dependencies not), one per line.
first_party_deps() {
    awk '/^\[/ { deps = ($0 ~ /dependencies\]$/ && $0 !~ /dev-dependencies/) }
         deps && match($0, /^[[:space:]]*diapason_[a-z_]+/) {
             name = substr($0, RSTART, RLENGTH); gsub(/[[:space:]]/, "", name); print name
         }' "$1" | sort -u
}

# Fail unless every workspace crate $1 depends on is in the allowed list that follows.
only_depends_on() {
    local manifest="$1" crate="$2"; shift 2
    local dep allowed
    for dep in $(first_party_deps "$manifest"); do
        for allowed in "$@"; do [[ "$dep" == "$allowed" ]] && continue 2; done
        violation "$crate depends on $dep" "$crate may depend only on: $*. See docs/adr/0022."
    done
}

# ── Rust: the session sits between ffi and the engine (adr/0022) ─────────────
before=$VIOLATIONS
only_depends_on rust/crates/session/Cargo.toml session diapason_engine diapason_audio_io
for crate in dsp engine audio_io; do
    if first_party_deps "rust/crates/$crate/Cargo.toml" | grep -qx diapason_session; then
        violation "$crate depends on session" "session sits above $crate, never beneath it."
    fi
done
[[ $VIOLATIONS -eq $before ]] && ok "session depends only on engine and audio_io, and nothing below it on session"

# ── Rust: ffi contains no logic ───────────────────────────────────────────────
# A proxy, not a proof: the FFI crate is type mapping over `session`, so it has no business
# depending on any other workspace crate or growing modules beyond the api surface.
before=$VIOLATIONS
only_depends_on packages/audio_engine/rust/Cargo.toml diapason_ffi diapason_session
[[ $VIOLATIONS -eq $before ]] && ok "ffi reaches Rust only through session"

printf '\n'
if [[ $VIOLATIONS -gt 0 ]]; then
    printf '%s✗ %d dependency-direction violation(s)%s\n\n' "$RED" "$VIOLATIONS" "$OFF"
    exit 1
fi
printf '%s✓ layer boundaries hold%s\n\n' "$GREEN" "$OFF"

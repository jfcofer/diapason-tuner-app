# Diapasón — the single command surface for humans, agents and CI.
#
# Rule: if CI runs it, it is a recipe here. If an agent needs to repeat it, it is a recipe here.
# Recipes marked [T-001] are the contract for the scaffold task and are not implemented yet.

set shell := ["bash", "-uc"]

default:
    @just --list

# ── Environment ───────────────────────────────────────────────────────────────

# Verify every pinned version in tools/versions.env. Run this first, always.
doctor:
    @tools/doctor.sh

# Prove doctor actually fails. Runs in a temp dir; never touches the working tree.
doctor-selftest:
    @tools/doctor-selftest.sh

# Clean clone → ready to work.
setup: doctor
    fvm install
    dart pub get                           # pub workspace resolves every member
    cargo fetch
    lefthook install
    just gen

# ── Code generation ───────────────────────────────────────────────────────────

# Formatting is part of generation, deliberately. FRB and rustfmt disagree about import ordering
# in frb_generated.rs, and that file is checked in (docs/REPO_LAYOUT.md). Without the format step
# `just check-drift` would oscillate forever: generate, reformat, diff, repeat.
gen: gen-frb gen-dart gen-fmt

gen-fmt:
    cargo fmt --all
    dart format .

gen-frb:
    flutter_rust_bridge_codegen generate

gen-dart:
    # build_runner must run inside each package that has a generator; running it at the workspace
    # root writes nothing. (--delete-conflicting-outputs was removed in current build_runner.)
    melos exec --depends-on=build_runner -- dart run build_runner build
    # flutter gen-l10n     # [T-0xx] re-enable when content strings land; the locale-aware app
    #                      # *label* lives in native strings.xml / InfoPlist.strings, not here.

# ── The gate ──────────────────────────────────────────────────────────────────

# Everything CI checks. Must be green before any work is called done.
verify: doctor-selftest fmt-check lint test check-drift check-deps docs-check

fmt-check:
    dart format --output=none --set-exit-if-changed .
    cargo fmt --all -- --check

fix:
    dart format .
    cargo fmt --all
    dart fix --apply

lint:
    # riverpod_lint is a first-party analyzer plugin (docs/adr/0014), so `analyze` reports it.
    # There is no separate custom_lint pass any more.
    flutter analyze --fatal-infos
    cargo clippy --workspace --all-targets -- -D warnings

test: test-rust test-dart

test-rust filter="":
    cargo nextest run --workspace {{filter}}
    # nextest cannot run doctests, and AGENTS.md §7 counts them as tests. Skipped when filtering,
    # because a nextest filter expression is not a doctest filter.
    {{ if filter == "" { "cargo test --workspace --doc" } else { "true" } }}

test-dart package="":
    @tools/test-dart.sh "{{package}}"      # [T-001] melos-scoped, or all packages if empty

# Goldens run only in the pinned container — see docs/TESTING.md §3.
goldens:
    @tools/goldens.sh check                # [T-001]

goldens-update:
    @tools/goldens.sh update               # [T-001]

# Regenerating must produce no diff.
check-drift: gen
    git diff --exit-code || (echo "Generated code is out of date. Run: just gen" && exit 1)

# Enforces the one-way dependency rule in AGENTS.md §5.
check-deps:
    @tools/check-deps.sh                   # [T-001]

# Link check, line budgets, ADR index consistency.
docs-check:
    @tools/docs-check.sh                   # [T-001]

# ── Running ───────────────────────────────────────────────────────────────────

run platform="ios" flavor="dev":
    cd apps/diapason && flutter run -d {{platform}} --flavor {{flavor}} \
        --dart-define-from-file=flavors/{{flavor}}.json

# ── Performance ───────────────────────────────────────────────────────────────

bench group="":
    cargo bench -p diapason_dsp {{group}}
    @tools/bench-compare.sh                # [T-001] fails on >10% regression vs baseline

bench-baseline:
    @tools/bench-compare.sh --save

size-report:
    cargo xtask size-report

# ── Release ───────────────────────────────────────────────────────────────────

build-android flavor="prod":
    cd apps/diapason && flutter build appbundle --flavor {{flavor}} \
        --dart-define-from-file=flavors/{{flavor}}.json

build-ios flavor="prod":
    cd apps/diapason && flutter build ipa --flavor {{flavor}} \
        --dart-define-from-file=flavors/{{flavor}}.json

# targetSdk 36, 16 KB page alignment, size budget. See docs/PLATFORM_AUDIO.md §2.
check-android-release:
    @tools/check-android-release.sh        # [T-001]

release-check: verify check-android-release
    @tools/release-check.sh                # [T-0xx, M7]

# ── Agent session protocol (AGENTS.md §2) ─────────────────────────────────────

session-start:
    @echo "Read, in order:"
    @echo "  1. AGENTS.md"
    @echo "  2. docs/agents/STATE.md"
    @echo "  3. the active task file named in STATE.md"
    @echo "  4. only the reference docs that task points to"
    @echo ""
    @git -c color.ui=always log --oneline -8
    @git status --short
    @just doctor

session-end:
    @tools/session-end.sh                  # [T-001] verifies STATE, task and journal were updated
    @just verify

# ── Maintenance ───────────────────────────────────────────────────────────────

clean:
    flutter clean
    cargo clean

clean-native:
    cargo clean -p diapason_ffi
    cd apps/diapason && flutter clean

rename name bundle_id:
    @tools/rename.sh "{{name}}" "{{bundle_id}}"   # [T-001]

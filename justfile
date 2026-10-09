# Diapasón — the single command surface for humans, agents and CI.
#
# Rule: if CI runs it, it is a recipe here. If an agent needs to repeat it, it is a recipe here.
# Recipes marked [T-001] are the contract for the scaffold task and are not implemented yet.

set shell := ["bash", "-uc"]

# Our Dart sources: tracked files minus cargokit's vendored `build_tool`, which is third-party code
# with its own pubspec and language version. Formatting it is not ours to do, and in a clean
# checkout (no `.dart_tool` inside it) the formatter cannot even parse it.
dart_files := "git ls-files -z '*.dart' ':!:packages/audio_engine/cargokit/**'"

default:
    @just --list

# ── Environment ───────────────────────────────────────────────────────────────

# Verify every pinned version in tools/versions.env. Run this first, always. With no arguments it
# checks everything; CI jobs name only the sections they installed, e.g. `just doctor flutter frb`.
doctor *sections:
    @tools/doctor.sh {{sections}}

# Prove doctor actually fails. Runs in a temp dir; never touches the working tree.
doctor-selftest *sections:
    @tools/doctor-selftest.sh {{sections}}

# Clean clone → ready to work.
setup: doctor
    fvm install
    dart pub get                           # pub workspace resolves every member
    cargo fetch
    lefthook install
    just gen

# What a fresh checkout needs before Dart can be analysed, tested or built: resolve the workspace
# and run Dart codegen. Riverpod's *.g.dart are generated, not committed (AGENTS.md §8), so without
# this the analyzer sees undefined providers. Every CI job that touches Dart starts here.
deps:
    flutter pub get
    just gen-dart

# ── Code generation ───────────────────────────────────────────────────────────

# Formatting is part of generation, deliberately. FRB and rustfmt disagree about import ordering
# in frb_generated.rs, and that file is checked in (docs/REPO_LAYOUT.md). Without the format step
# `just check-drift` would oscillate forever: generate, reformat, diff, repeat.
gen: gen-frb gen-dart gen-fmt

gen-fmt:
    cargo fmt --all
    {{dart_files}} | xargs -0 dart format

gen-frb:
    flutter_rust_bridge_codegen generate

gen-dart:
    # build_runner must run inside each package that has a generator; running it at the workspace
    # root writes nothing. (--delete-conflicting-outputs was removed in current build_runner.)
    # melos is a workspace dev_dependency, so `dart run` uses the locked version - no global install.
    dart run melos exec --depends-on=build_runner -- dart run build_runner build
    # flutter gen-l10n     # [T-0xx] re-enable when content strings land; the locale-aware app
    #                      # *label* lives in native strings.xml / InfoPlist.strings, not here.

# ── The gate ──────────────────────────────────────────────────────────────────

# Everything CI checks. Must be green before any work is called done. Each CI job runs a subset of
# these recipes, never a command of its own (docs/CI_RELEASE.md §1).
verify: doctor-selftest fmt-check lint test deny doc-rust check-drift check-deps docs-check lint-ci ios-project-check

fmt-check: fmt-check-dart fmt-check-rust

fmt-check-dart:
    {{dart_files}} | xargs -0 dart format --output=none --set-exit-if-changed

fmt-check-rust:
    cargo fmt --all -- --check

fix:
    {{dart_files}} | xargs -0 dart format
    cargo fmt --all
    dart fix --apply

lint: lint-dart lint-rust

# riverpod_lint is a first-party analyzer plugin (docs/adr/0014), so `analyze` reports it.
lint-dart:
    flutter analyze --fatal-infos

lint-rust: && lint-rust-android
    cargo clippy --workspace --all-targets -- -D warnings

# Advisories, licences, bans, sources - configured in deny.toml.
deny:
    cargo deny check

# Public items must be documented (AGENTS.md §7); a broken intra-doc link is an error.
doc-rust:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# CI configuration and shell scripts are code: lint them like code. actionlint also runs
# shellcheck over every workflow `run:` block.
lint-ci:
    actionlint
    shellcheck tools/*.sh

test: test-rust test-dart

test-rust filter="":
    cargo nextest run --workspace {{filter}}
    # nextest cannot run doctests, and AGENTS.md §7 counts them as tests. Skipped when filtering,
    # because a nextest filter expression is not a doctest filter.
    {{ if filter == "" { "cargo test --workspace --doc" } else { "true" } }}

test-dart package="":
    @tools/test-dart.sh "{{package}}"      # [T-001] melos-scoped, or all packages if empty

# Goldens are authoritative only on the pinned CI runner - see docs/adr/0016.
goldens:
    @tools/goldens.sh check                # [T-001]

goldens-update:
    @tools/goldens.sh update               # [T-001]

# Regenerating must produce no diff. Scoped to the paths `gen` writes: a bare `git diff` here
# reports every uncommitted edit as "codegen drift", which is a lie with a confusing fix attached.
check-drift: gen
    @git diff --exit-code -- '**/frb_generated*.dart' '**/frb_generated.rs'         'packages/*/lib/src/rust/**'         || (echo "Generated code is out of date. Run: just gen" && exit 1)

# Enforces the one-way dependency rule in AGENTS.md §5.
check-deps:
    @tools/check-deps.sh                   # [T-001]

# Link check, line budgets, ADR index consistency.
docs-check:
    @tools/docs-check.sh                   # [T-001]

# ── Running ───────────────────────────────────────────────────────────────────

run platform="ios" flavor="dev":
    cd apps/diapason && flutter run -d {{platform}} --flavor {{flavor}} \
        --target lib/main_{{flavor}}.dart --dart-define-from-file=flavors/{{flavor}}.json

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
        --target lib/main_{{flavor}}.dart --dart-define-from-file=flavors/{{flavor}}.json

# A signed, distributable IPA. Needs a development team and profiles (M7, docs/CI_RELEASE.md).
build-ios flavor="prod":
    cd apps/diapason && flutter build ipa --flavor {{flavor}} \
        --target lib/main_{{flavor}}.dart --dart-define-from-file=flavors/{{flavor}}.json

# A release-mode device build with signing off: everything compiled and linked, nothing to ship.
# This is what CI builds, because `build ipa` treats a team-less archive as a failure.
build-ios-unsigned flavor="prod":
    cd apps/diapason && flutter build ios --release --no-codesign --flavor {{flavor}} \
        --target lib/main_{{flavor}}.dart --dart-define-from-file=flavors/{{flavor}}.json

# Privacy manifest bundled, mic usage string, MinimumOSVersion, Rust engine actually linked.
check-ios-release:
    @tools/check-ios-release.sh

# The iOS flavour wiring (configurations, schemes, xcconfigs, Podfile, privacy manifest) is
# generated, never hand-edited - there is no Xcode on this project. Needs the `xcodeproj` gem.
ios-project:
    ruby tools/ios/configure_project.rb

# What tools/ios/configure_project.rb owns - and nothing else, so a hand edit to Info.plist or
# Swift is never misreported as generator drift with a fix that would not fix it.
ios_generated := "'apps/*/ios/Runner.xcodeproj' 'apps/*/ios/Flutter/*.xcconfig' 'apps/*/ios/Podfile'"

# CI: regenerating the iOS project must change nothing it owns, tracked or untracked.
ios-project-check: ios-project
    @test -z "$(git status --porcelain -- {{ios_generated}})" \
        || (git status --short -- {{ios_generated}}; echo "iOS project drifted. Run: just ios-project" && exit 1)

# `--release` measures callback timing instead; it has no allocation trap, so no canary.
# audio_io's conformance suite and allocation canary on a connected Android device (T-002b).
test-android-device *args:
    @tools/test-android-device.sh {{args}}

# No NDK needed: nothing is linked, and rust-toolchain.toml installs the target.
# Clippy and rustdoc for the Android-only code, which no host build compiles.
lint-rust-android:
    cargo clippy --target aarch64-linux-android -p diapason_audio_io --all-targets \
        --features conformance -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --target aarch64-linux-android -p diapason_audio_io \
        --no-deps --features conformance

# targetSdk 36, 16 KB page alignment, size budget. See docs/PLATFORM_AUDIO.md §2.
check-android-release:
    @tools/check-android-release.sh        # [T-001]

release-check: verify check-android-release
    @tools/release-check.sh                # [T-0xx, M7]

# ── Agent session protocol (AGENTS.md §2) ─────────────────────────────────────

session-start:
    @tools/session-start.sh                # [T-007] reading order, unmerged branches, open PRs, doctor

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

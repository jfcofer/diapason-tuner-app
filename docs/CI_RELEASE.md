# CI, flavours and release

This document owns **everything between a commit and a user**.

## 1. Pipelines

`.github/workflows/ci.yml` — on every PR and push to `main`. Every step is a `just` recipe:

| Job | Runner | Recipes | Blocking |
|---|---|---|---|
| `rust` | ubuntu-24.04 | `fmt-check-rust`, `lint-rust` (clippy `-D warnings`), `test-rust` (nextest + doctests), `deny`, `doc-rust` | Yes |
| `dart` | ubuntu-24.04 | `fmt-check-dart`, `lint-dart` (analyze incl. riverpod_lint, `adr/0014`), `test-dart` | Yes |
| `goldens` | ubuntu-24.04 | `goldens` (`adr/0016`) | Yes |
| `checks` | ubuntu-24.04 | `doctor-selftest`, `check-drift`, `ios-project-check`, `check-deps`, `docs-check`, `lint-ci` | Yes |
| `build-android` | ubuntu-24.04 | `build-android prod`, `check-android-release` (targetSdk, 16 KB alignment, size) | Yes |
| `build-ios` | macos-26 | `build-ios-unsigned prod`, `check-ios-release` (privacy manifest *in the .app*, mic string, MinimumOSVersion, Rust linked) | Yes |
| `bench` | — | Criterion vs. committed baselines, ≥ 10 % regression fails | Not yet: nothing to measure until `T-003` |
| `integration` | — | Lifecycle matrix on device/simulator | Not yet: `T-002` |
| `miri` | — | `ffi` and `audio_io` unsafe blocks | Not yet: no `unsafe` exists |

Everything a job runs is a `just` recipe. CI never contains logic that a developer or an agent
cannot reproduce with one command — that is the whole point of the justfile.

**Toolchain.** `.github/actions/toolchain` installs what each job asks for, from
`tools/versions.env` and `rust-toolchain.toml`; no version is written in `.github/`. Each Linux job
then runs `just doctor <sections>` on exactly what it installed. `build-ios` skips doctor (it needs
GNU grep) and relies on the action installing the pins by construction.

**Supply chain.** Third-party actions are pinned by commit SHA with the release in a comment;
`permissions: contents: read` at the top. Dependabot (`.github/dependabot.yml`) proposes weekly
grouped bumps for actions, Cargo and pub; toolchain pins and flutter_rust_bridge are excluded,
because those move only with an ADR.

Caching: `Swatinem/rust-cache` (registry, `target/`, and `~/.cargo/bin`, so the FRB codegen install
is a no-op when warm) and flutter-action's SDK and pub caches.

## 2. Flavours

| Flavour | Bundle ID | Purpose |
|---|---|---|
| `dev` | `com.example.diapason.dev` | Local, debug logging, on-screen engine diagnostics |
| `stg` | `com.example.diapason.stg` | Internal testers, release build, diagnostics still available |
| `prod` | `com.example.diapason` | Store |

All three install side by side. Configuration comes from `--dart-define-from-file=flavors/<f>.json`;
there is no `.env` and no runtime configuration file.

## 3. Versioning

Conventional Commits → `git-cliff` generates `CHANGELOG.md`. Semantic version in `pubspec.yaml`;
build number is the CI run number, never hand-edited. A release is a tag `v1.2.3` on `main`, which
triggers `release.yml`.

## 4. Signing and secrets

iOS: `fastlane match` with a private certificates repo; App Store Connect API key as a GitHub
Actions secret in a protected environment. Android: upload keystore in the same protected
environment, base64-encoded, and Play App Signing enabled so the upload key is replaceable.

No secret exists in the repository. No agent has access to the release environment — agents build
unsigned artifacts only, and this is enforced by the environment protection rules rather than by
policy. Getting this boundary right is what makes it safe to let an agent run CI at all.

## 5. Store submission

Blocking checklist, automated where possible (`just release-check`):

- Android targets API 36 (required for submission from 31 Aug 2026), min 26, 16 KB pages verified,
  AAB under budget, Data Safety form declares **no data collected**, foreground-service type
  declared and justified in the listing.
- iOS ships `PrivacyInfo.xcprivacy` with tracking false and required-reason APIs declared,
  `NSMicrophoneUsageDescription` localised in both languages, background audio mode declared and
  used, screenshots for every required device class.
- Both: all strings localised (en + es) with no fallback leaks, accessibility pass complete,
  the manual verification matrix in `TESTING.md` §7 signed off, crash-free session rate from the
  staging cohort above 99.5 %.

## 6. Observability

No telemetry by default. Crash reporting is opt-in at first launch, off by default, and if enabled
sends crashes only — no usage events, no identifiers, no session data. This is the only network call
the app may ever make, it is gated behind an explicit choice, and adding a second one requires an
ADR and a store-listing change.

Locally and in `dev`/`stg`, the engine exposes a diagnostics overlay: measured latency, callback
worst-case duration, xrun count, chosen input preset, actual sample rate and buffer size. Ship it
disabled in `prod` behind a hidden gesture — it is how a support request from a user with an unusual
device gets resolved in one round trip instead of five.

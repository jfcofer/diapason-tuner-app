# CI, flavours and release

This document owns **everything between a commit and a user**.

## 1. Pipelines

`.github/workflows/ci.yml` — on every PR and push to `main`:

| Job | Runs | Blocking |
|---|---|---|
| `rust` | fmt, clippy `-D warnings`, nextest, `cargo deny`, doc build | Yes |
| `dart` | `dart format --set-exit-if-changed`, `flutter analyze`, `custom_lint`, unit + widget tests | Yes |
| `goldens` | Golden tests inside the pinned Linux container | Yes |
| `codegen-drift` | `just gen` then `git diff --exit-code` | Yes |
| `deps` | Dependency-direction check (`just check-deps`) | Yes |
| `docs` | Link check, line budgets, ADR index consistency | Yes |
| `build-android` | Release AAB, then `just check-android-release` (targetSdk, 16 KB alignment, size) | Yes |
| `build-ios` | Release archive, privacy-manifest presence check | Yes |
| `bench` | Criterion vs. committed baselines, ≥ 10 % regression fails | Yes |
| `integration` | Device-farm run of the lifecycle matrix | Nightly, non-blocking on PR |
| `miri` | `ffi` and `audio_io` unsafe blocks | Weekly |

Everything a job runs is a `just` recipe. CI never contains logic that a developer or an agent
cannot reproduce with one command — that is the whole point of the justfile.

Caching: `Swatinem/rust-cache` keyed on `Cargo.lock` + target triple, and the pub cache keyed on
`pubspec.lock`. Cold CI on this stack is ~20 minutes; warm should be under 8.

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

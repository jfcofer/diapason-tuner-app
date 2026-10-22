---
id: T-013
title: Run device tests with patrol, so the OS side of a lifecycle row is tested
status: in-progress
milestone: M1
owner: claude
created: 2026-10-21
---

## Goal

`T-002d` must prove the microphone is released when the app is backgrounded, and that the
permission dialog leads where it should. `integration_test` alone can only fake an
`AppLifecycleState` through the binding, which tests our reaction but not the OS's. Patrol presses
Home and recents and answers system dialogs from inside the test. `TESTING.md` §3 already names it;
nothing installs, pins or runs it. Adopt it (owner's choice, 2026-10-21) so that the device tests
`T-002d` writes run on the Redmi, and cannot rot or leak into a release build.

## Context

Read: `docs/TESTING.md` §3, `tools/test-integration-android.sh`, `docs/adr/0011` (pins),
`docs/adr/0024` (an exact pin when a tool's output varies), `AGENTS.md` §8 (zero network).

- **Versions (checked 2026-10-21):** patrol 4.10.0 (2026-09-15) with patrol_cli 4.8.0; the
  compatibility table pairs CLI 4.7.0+ with patrol 4.9.0+, Flutter ≥ 3.32 (we pin 3.47.1).
- **Android setup (upstream docs):** `testInstrumentationRunner =
  "pl.leancode.patrol.PatrolJUnitRunner"`, `clearPackageData`, the AndroidX test orchestrator
  (`androidTestUtil("androidx.test:orchestrator:1.5.1")`), and a parameterised `MainActivityTest`
  in `src/androidTest/`. `build.gradle.kts` holds the signing block, so its edit asks the owner.
- **Patrol 4.10's mobile automator** has `pressHome`, `openApp`, `pressRecentApps`, and permission
  grant / deny. It has **no lock-screen call** and no way to revoke in Settings.
- **HyperOS (the Redmi)** refuses shell `pm grant`/`revoke`/`uninstall`, and USB installs wait for
  a tap. Whether the orchestrator's `pm clear` survives that is unknown: measure before deciding.
- Patrol talks to the app over a localhost port, so its debug and test builds declare INTERNET.
  The release build must not (`AGENTS.md` §8): prove it on the shipped artifact.

## Acceptance criteria

- [x] A Redmi spike answers, in the journal: does `clearPackageData` work under HyperOS; how many
      install taps a run costs; do patrol's permission-dialog selectors match HyperOS's dialog
- [x] `adr/0026` records the adoption, the pins, the setup that shipped, and what was rejected
- [x] `PATROL_VERSION` and `PATROL_CLI_VERSION` in `tools/versions.env`; `apps/diapason` pins the
      package exactly; `just doctor` fails on a CLI or package that disagrees, and
      `doctor-selftest` proves it; `just setup` installs the pinned CLI. **Deviation:** nothing to
      install: the CLI is a dev dependency run from the lockfile, like melos
- [x] Dependabot never bumps `patrol` alone (it moves with the CLI pin)
- [x] `just test-integration-android` runs the engine contract through `patrol test` on the Redmi,
      with and without the microphone
- [ ] CI compiles the patrol test APK in the `build-android` job (still six jobs), with a `just`
      recipe; the minutes it adds are recorded
- [x] `check-android-release` fails a release artifact that contains patrol classes or declares
      `android.permission.INTERNET`
- [x] `TESTING.md` §3 says what patrol covers and what stays manual; `DEVELOPMENT.md` lists the CLI
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

The lifecycle tests themselves (`T-002d`). iOS patrol setup (`T-002c`: it needs the Simulator job
and its own Xcode target). Cloud device farms.

## Implementation notes

- **The CLI is not installed globally.** `patrol_cli` is an exact dev dependency of
  `apps/diapason`, run as `dart run patrol_cli:main`, so `pubspec.lock` is the install and doctor
  checks it (as melos, `adr/0006`). `tools/patrol.sh` is the one entry point.
- **Two traps found in the spike**, both handled in `tools/patrol.sh`: the CLI writes completion
  setup into `~/.bashrc` on its first run (`PATROL_NO_COMPLETION` stops it; the block it wrote here
  was removed with its own `uninstall-completion-files`), and with Chrome attached as a second
  device it re-asks "select an option" forever on a closed stdin (400 MB of log).
- **`integration_test/` is gone**, with its dev dependency: the contract runs as two patrol tests,
  denied then allowed, each from cleared data. patrol_cli writes `patrol_test/test_bundle.dart`
  on each run; it is gitignored and excluded from analysis.
- **CI compiles, does not run:** `just build-android-tests` compiles the androidTest Kotlin
  against the dev entry point (~30 s locally, no Rust build).
- **patrol 4.10 applies KGP**, which Flutter 3.47 warns a future Flutter will refuse (`adr/0026`).

## Verification performed

- **Spike, Redmi 23117RA68G (Android 16), 2026-10-21:** both contract tests pass (deny then
  allow), 81 s with the build; swapped order (allow then deny) passes too, 52 s, which proves
  `clearPackageData` isolates them. HyperOS asks for every USB install even with "USB debugging
  (Security settings)" on (`persist.security.adbinput=1`); unanswered, the run fails as
  `INSTALL_FAILED_USER_RESTRICTED`, which the runner now names.
- **Release guard, both directions:** a debug APK renamed `*-release.apk` fails it twice (INTERNET,
  patrol code); a clean release is proved by CI's `build-android`.
- `just doctor-selftest flutter`: `patrol_cli lockfile drift` caught.
- **The final runner**, `just test-integration-android`, Redmi, 2026-10-22: 2/2 in 62 s, with the
  owner approving the two install prompts (two earlier attempts unanswered: refused, and named).
  After review made the dialog's appearance an explicit assertion: 2/2 in 49 s.
- `just verify`: green but for STATE's line budget, fixed at session end.

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

- [ ] A Redmi spike answers, in the journal: does `clearPackageData` work under HyperOS; how many
      install taps a run costs; do patrol's permission-dialog selectors match HyperOS's dialog
- [ ] `adr/0026` records the adoption, the pins, the setup that shipped, and what was rejected
- [ ] `PATROL_VERSION` and `PATROL_CLI_VERSION` in `tools/versions.env`; `apps/diapason` pins the
      package exactly; `just doctor` fails on a CLI or package that disagrees, and
      `doctor-selftest` proves it; `just setup` installs the pinned CLI
- [ ] Dependabot never bumps `patrol` alone (it moves with the CLI pin)
- [ ] `just test-integration-android` runs the engine contract through `patrol test` on the Redmi,
      with and without the microphone
- [ ] CI compiles the patrol test APK in the `build-android` job (still six jobs), with a `just`
      recipe; the minutes it adds are recorded
- [ ] `check-android-release` fails a release artifact that contains patrol classes or declares
      `android.permission.INTERNET`
- [ ] `TESTING.md` §3 says what patrol covers and what stays manual; `DEVELOPMENT.md` lists the CLI
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

The lifecycle tests themselves (`T-002d`). iOS patrol setup (`T-002c`: it needs the Simulator job
and its own Xcode target). Cloud device farms.

## Implementation notes

## Verification performed

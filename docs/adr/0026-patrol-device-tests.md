# 0026 — Device tests run through patrol, pinned with its CLI, never shipped

**Status:** Accepted · 2026-10-21

## Context

`T-002d` must show that the microphone is released when the app leaves the screen, and that the
permission dialog leads where it should. `integration_test` can only fake an `AppLifecycleState`
through the binding: that tests our reaction, not what the OS does. It cannot press Home, open
recents, or answer a system dialog. On the Redmi the dialog was a person's job, because HyperOS
refuses the shell's `pm grant` and `pm revoke` (`T-002b`).

`TESTING.md` §3 named patrol for exactly this, and nothing installed, pinned or ran it. The owner
chose to adopt it (2026-10-21) over driving the binding alone.

## Decision

- **Patrol runs every on-device Dart test**, in `apps/diapason/patrol_test/`, through
  `just test-integration-android`. The engine contract moved there from `integration_test/`.
- **patrol 4.10.0 and patrol_cli 4.8.0, exactly, and only together.** Patrol's compatibility table
  pairs CLI 4.7.0+ with patrol 4.9.0+ (Flutter ≥ 3.32). Both are dev dependencies of
  `apps/diapason`, so the CLI runs as `dart run patrol_cli:main`, like melos (`adr/0006`): the
  lockfile is the install. `PATROL_VERSION` and `PATROL_CLI_VERSION` in `tools/versions.env` are
  the pins (`adr/0011`), doctor checks the lockfile against them, and Dependabot ignores both.
- **`tools/patrol.sh` is the only way in.** It sets `PATROL_NO_COMPLETION`, because on its first
  run patrol_cli otherwise writes shell-completion setup into the user's `~/.bashrc` (it did, on
  2026-10-21; removed with its own `uninstall-completion-files`). It always passes `--device`,
  because with a second device attached (a desktop Chrome counts) the CLI asks which one and, with
  no terminal, repeats the question forever: 400 MB of log in two minutes.
- **Android setup as upstream documents it:** the `PatrolJUnitRunner`, `clearPackageData`, and the
  AndroidX test orchestrator (`androidx.test:orchestrator:1.5.1`). `MainActivityTest` is a Kotlin
  port of upstream's Java template, so ktfmt formats it (`adr/0023`).
- **Never shipped.** Patrol reaches the app over a localhost port, so its debug and test builds
  declare INTERNET. `check-android-release` fails a release artifact that declares
  `android.permission.INTERNET` or contains patrol code (`AGENTS.md` §8). Each scan first proves it
  read something (RECORD_AUDIO seen; a dex found), so an empty scan cannot pass.
- **Measured on the Redmi before deciding (2026-10-21):** patrol's dialog selectors match
  HyperOS's microphone dialog (deny and allow); `clearPackageData` isolates tests (with the order
  swapped, the deny test after an allow still saw a fresh dialog); a run is 52–81 s once built.
  HyperOS still asks a person to approve **every** USB install, even with "USB debugging (Security
  settings)" on; unanswered, it fails as `INSTALL_FAILED_USER_RESTRICTED`.
- **What stays manual:** patrol 4.10 has no lock-screen call and cannot revoke a permission in
  Settings (which kills the app on Android anyway). Those rows are recorded with device and OS.

## Consequences

**Good.**
- Home, recents and the microphone dialog are driven from the test, on the real engine.
- No `pm grant`/`pm revoke`: the runner lost its vendor-specific fallback. On HyperOS a person
  still approves the installs at the start of a run; nothing during it.

**Bad.**
- A second toolchain piece to pin and to move in lockstep.
- **patrol 4.10 applies the Kotlin Gradle Plugin.** Flutter 3.47 warns that a future Flutter will
  refuse to build such plugins ("migrate to Built-in Kotlin"). A Flutter bump must check patrol
  first, and may have to wait for it.
- Each test runs in its own instrumentation with cleared data, so a run is slower than one
  `flutter test` process, and test output arrives through Gradle, not the Dart test runner.
- CI compiles the test runner's Kotlin (`just build-android-tests`, ~30 s) but runs no device, so
  only `just test-integration-android` on a device proves the tests pass.

**Rejected.**
- **The binding alone** (`handleAppLifecycleStateChanged` in `integration_test`): it tests that we
  react to a lifecycle state, not that the OS sends it or that the microphone is really released.
- **adb-scripted backgrounding** around `flutter test`: the script and the test cannot synchronise
  without a channel of their own, which is what patrol already is.
- **A global `dart pub global activate patrol_cli`:** a second, unpinned copy that drifts from the
  lockfile, and a PATH entry the repo cannot check.

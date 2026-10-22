---
id: T-002d
title: Release the microphone whenever the tuner is not in front of the user
status: todo
milestone: M1
owner: unassigned
created: 2026-10-20
---

## Goal

Once the tuner listens, the microphone stays open until the process dies: across screens, in the
background, and with the screen locked. In the background the platform silences it, so the tuner
hears exact zeros and cannot say why. That is a privacy defect (the OS microphone indicator stays
lit), a battery cost, and a broken M1 exit criterion ("stream survives every row of the lifecycle
matrix"). Make the Tuner column of `PLATFORM_AUDIO.md` §5 true on Android.

## Context

Read: `docs/PLATFORM_AUDIO.md` §5 and §6, `docs/adr/0022` (every (re)open replays the desired
state), `docs/adr/0024`, `docs/TESTING.md` §1 and §3. A slice of `T-002`, split out of `T-002b`
part 3.

- `EngineHandle` already has `start(input:)` and `stop()`, so the likely shape is Dart-only:
  `TunerController` owns "the user wants to listen", and an `AppLifecycleListener` in
  `feature_tuner` (never `core_ui`, `AGENTS.md` §5) stops on `hidden`/`paused` and restarts on
  `resumed` if the user still wants to. If a new session `Command` turns out to be needed, it gets a
  replay test (`adr/0022`) and the FFI change goes through plan mode.
- Leaving the tuner route stops the stream too, unless the dev A4 tone is playing.
- Revocation in Settings **kills the app** on the Redmi (verified in `T-002b`), so "show recovery
  UI" means: on the next launch, the permission check shows recovery, not a crash.
- "Another app takes exclusive audio": on Android 10+, concurrent capture silences one client
  rather than failing it (the input keeps delivering zeros). Detecting that is an open question.
  Measure on the Redmi before designing.
- **Patrol is adopted first, in `T-013`** (owner's choice, 2026-10-21): it presses Home and
  recents and answers the permission dialog. It cannot lock the screen or revoke in Settings, so
  those rows are manual, recorded with device and OS.
- A call and another app capturing both silence our input (Android 10+).
  `AudioRecordingConfiguration.isClientSilenced()` (API 29) may detect both: measure, then detect
  (owner, 2026-10-21).

## Acceptance criteria

- [ ] Backgrounded (home, app switcher, screen locked): the stream stops and the microphone is
      released. On the Redmi, `dumpsys media.audio_policy` shows no capture client from the app
      within 1 s
- [ ] Foregrounded: listening resumes only if it was active before; the permission is re-checked
- [ ] Navigating away from the tuner stops the stream; returning does not start it unasked
- [ ] Headphones unplugged / Bluetooth connected mid-session: the tuner keeps listening (rebuild
      already done in `T-002b`; re-verified here with the lifecycle code in place)
- [ ] Permission revoked in Settings, then the app reopened: the recovery UI, no crash
- [ ] Phone call begins: the tuner stops; the call ends: it resumes only if it was listening
- [ ] Another app capturing: behaviour measured on the Redmi and recorded; the tuner says so if it
      can be detected, or the limit is recorded in `PLATFORM_AUDIO.md` §5 if it cannot
- [ ] Automatable rows are in `apps/diapason/patrol_test/lifecycle_test.dart`, run by
      `just test-integration-android`; the manual rows are recorded with device and OS
- [ ] View-model tests against `FakeEngine` for every transition, with `fakeAsync` (no wall clock)
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

Metronome-column rows (M4: FGS, MediaSession, lock-screen controls). Low-power 20 Hz snapshots and
the 60 s silence stop (`PLATFORM_AUDIO.md` §6; tuner analysis does not exist until M2/M3). iOS.

## Implementation notes

## Verification performed

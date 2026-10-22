---
id: T-002d
title: Release the microphone whenever the tuner is not in front of the user
status: in-progress
milestone: M1
owner: claude
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

- [x] Backgrounded (home, app switcher, screen locked): the stream stops and the microphone is
      released. On the Redmi, `dumpsys media.audio_policy` shows no capture client from the app
      within 1 s
- [x] Foregrounded: listening resumes only if it was active before; the permission is re-checked
- [x] Navigating away from the tuner stops the stream; returning does not start it unasked
- [ ] Headphones unplugged / Bluetooth connected mid-session: the tuner keeps listening (rebuild
      already done in `T-002b`; re-verified here with the lifecycle code in place)
- [x] Permission revoked in Settings, then the app reopened: the recovery UI, no crash
- [ ] Phone call begins: the tuner stops; the call ends: it resumes only if it was listening
- [ ] Another app capturing: behaviour measured on the Redmi and recorded; the tuner says so if it
      can be detected, or the limit is recorded in `PLATFORM_AUDIO.md` §5 if it cannot
- [ ] Automatable rows are in `apps/diapason/patrol_test/lifecycle_test.dart`, run by
      `just test-integration-android`; the manual rows are recorded with device and OS
- [x] View-model tests against `FakeEngine` for every transition, with `fakeAsync` (no wall clock)
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

Metronome-column rows (M4: FGS, MediaSession, lock-screen controls). Low-power 20 Hz snapshots and
the 60 s silence stop (`PLATFORM_AUDIO.md` §6; tuner analysis does not exist until M2/M3). iOS.

## Implementation notes

**Part 1 design** (2026-10-22, from the plan approved 2026-10-21, checked against the live code).
Dart only: no new session `Command`, no FFI change. The existing `stop()`, `start(input:)` and
`startTone`/`stopTone` give every transition, so `adr/0022` needs no new replay test.

- A pure `engineTarget(...)` in `feature_tuner` maps the user's intent (listen, tone), the
  permission, foreground and visibility onto `stopped`, `outputOnly` or `duplex`. `inactive`
  counts as foreground: the permission dialog only makes the app inactive.
- `core_platform` gains an `AppLifecycle` seam (`FlutterAppLifecycle`, `FakeAppLifecycle`), as
  `MicrophonePermission` has; the metronome will need it in M4.
- `TunerController` owns the intent, applies the target on every change, re-reads the permission
  on return from the background, and stops everything when disposed. Visibility comes from
  `ModalRoute.isCurrentOf` and `TickerMode.valuesOf` (`TickerMode.of` is deprecated in 3.47.1).
- The controller tracks the tone itself: after `stop()` the snapshot says no tone, but the session
  still holds it and would replay it on the next start.
- **Deviation:** leaving the tuner stops the dev A4 tone too, instead of keeping it. A tone kept
  past the tuner either outlives its owner in the background or is replayed by the metronome's
  first `start()`. It is development scaffolding, so nothing user-facing is lost.
- A granted permission with no stream showed no control at all; the panel now has Start and Stop
  listening, the user's own control of the microphone.
- On build only `granted` and `permanentlyDenied` are taken from `status()`: Android reports
  "never asked" as denied, and that must still show the explanation first.


**Built (part 1, 2026-10-22).**
- **Visibility is `TickerMode` alone, not `ModalRoute.isCurrentOf`.** The review found that
  `isCurrentOf` is false under every popup route. A dialog, a menu, or the bottom sheet that
  `DESIGN_SYSTEM.md` gives the compact controls would then have stopped the tuner. An opaque route
  or an offstage branch stops its tickers; a popup does not.
- **The matrix (`PLATFORM_AUDIO.md` §5) has IDs L1 to L11, and `docs-check` checks its last
  column.** L11, leaving the screen, is new.
  - A `patrol` cell needs the device, the OS and the date of a passing run, because CI has no
    device. So L1, L2 and L11 stay `open` until the Redmi runs them.
- **`diapasonRoot()` in `bootstrap.dart`** is what the patrol tests pump, so they run the app with
  the same overrides it ships with.
- **`just mic-clients`**, written once the Redmi was attached and its two `dumpsys` formats had
  been read. The `media.audio_policy` Inputs section gives the open AudioRecord clients; Outputs
  lists AudioTrack clients in the same format, so it is excluded. The `dumpsys audio`
  recording-event log gives millisecond timestamps.
- **`tools/patrol.sh` sets `PATROL_ANALYTICS_ENABLED=false` and `CI`.**
  - patrol_cli had been posting usage analytics to Google Analytics from this host since T-013.
    It reads the variable, then its config file, then defaults to on.
  - It also checks pub.dev for a newer CLI before every command, and failed offline. The CLI is
    pinned, so the check is only done on CI.
- **Open for the owner:** L11 (returning does not listen) contradicts `PRODUCT_SPEC.md` §1 and §2
  ("opens … listening"). It is kept for M1, and logged in STATE's open questions for M3.

## Verification performed

- 2026-10-22, host only. `just verify` passed. The tuner has 36 tests and the policy is tested on
  all 32 input combinations.
- Each race test was checked by putting the bug back: the test then fails.
- `docs-check` was checked the same way, on purpose-broken matrices: no ID, a malformed ID, a bare
  `patrol`, `deferred: M1`, a duplicate ID, a missing task, and a renamed heading. Each one fails.
- 2026-10-22, Redmi 23117RA68G, Android 16 (HyperOS). `just test-integration-android`: 5/5 pass.
  These are the two contract tests, plus L1/L2 by Home, L1/L2 by recents, and L11.
- **Manual timings,** from the `wm_on_*_called` lines of the activity's events log against the
  `rec` events in `just mic-clients`:

  | Row | Action | Released | Measured from |
  |---|---|---|---|
  | L1 | Home | 0.89 s | the key press |
  | L1 | Home | 50 ms | `onStop` |
  | L1 | Recents | 38 ms | `onStop` |
  | L8 | Lock | 142 ms | the key press |

  **Recents:** HyperOS stops the activity behind the app switcher, so the app is hidden, not
  inactive. Android itself took 1.05 s from pause to stop there.
- **L2:** resumed, then the microphone reopened 0.42 s later. After a screen unlock it reopened
  too.
- **Not listening, then Home and back:** no capture client.
- **Stop listening:** the capture client is gone, and so is the green privacy dot.
- **L7:** `pm revoke` killed the process. The relaunch showed the explanation, with Start
  listening asking again, and did not crash.
- **A false alarm:** a "permission dialog" in one UI dump was a stale `/sdcard/ui.xml`.
  `uiautomator dump` fails ("could not get idle state") while the tuner redraws at 30 Hz.
- **For part 2:** an earlier install's last session shows `rec update … silenced` just before
  `release`, so this ROM reports silencing.

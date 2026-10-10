# 2026-10-09 — capabilities

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2b-i, closes T-011  **Milestone:** M1

## Done
- Closed T-011, after the owner merged #15. Dependabot #2 and #10 merged too.
- **2b-i** (`adr/0024`):
  - `core_platform` is a plugin, with a Pigeon channel that reports the device's audio
    capabilities;
  - `diapason_session::choose_input_preset` picks the preset from them;
  - Dart tells the session the microphone permission (`MicrophoneAccess`), so a denied microphone
    is never opened and is reported as `PermissionDenied`;
  - the snapshot carries the requested preset beside the obtained one;
  - the engine contract now asserts the exact fault.
- Fixed a 2a gap found on the way: the "dev-only" A4 tone button showed in every flavour. It is
  now behind `showEngineDiagnosticsProvider`, which defaults to off.

## Tried and abandoned
- **Passing the native rate and burst to Rust**, as the approved plan said. AAudio already picks
  both, and Rust would not use them, so only the two facts the preset rule reads cross the bridge.
- **Committing Pigeon's output**, as frb's is. Pigeon runs wherever `pub get` has run, like
  `build_runner`, so its output joins the other generated files `just deps` produces.

## Surprises
- **The review caught a bug that the fake had hidden.** If the contract reported the denial before
  asking for the microphone, the real supervisor never named the permission: `start()` wiped the
  stored fault, and `tick` saw nothing to rebuild. The fake recomputed its fault on every call, so
  it passed. Now:
  - the fault is derived;
  - a driver test replays the contract's order on the host, and was shown to fail without the fix;
  - the fake follows the same grant rule.
  Lesson: a fake that recomputes can hide a real one's ordering bug, so the contract's call order
  needs replaying on the host, not only on a device.
- **Flutter 3.47's plugin template applies no Kotlin Gradle plugin**, and its example app sets
  `android.builtInKotlin=false`, as ours does. The template was copied rather than guessed.
  The device build is what proves it.
- **`case` binds looser than `&&` in a collection `if`.** `if (flag && x case final y?)` does not
  parse as intended. Use `if (x case final y? when flag)`.
- **A commit failed at random with exit 128.** lefthook ran the format jobs in parallel, each
  running `git add`, and they raced for the index lock. They now run in sequence (`lefthook.yml`).
- **Script edits skip the format hook.** `fmt-check-dart` caught one in `verify`.

## Left for next session
- The owner: run `just test-integration-android` on the Redmi (installs need a tap). During the
  granted half, run `adb shell -n dumpsys media.audio_policy` to see which recording source was
  applied. Record both here.
- Then the PR for 2b-i, and 2b-ii.

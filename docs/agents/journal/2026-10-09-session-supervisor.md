# 2026-10-09 — session-supervisor

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2a  **Milestone:** M1

## Done
- **`adr/0022` and `rust/crates/session`.**
  - A sans-IO `Supervisor` on virtual time, driven by one thread.
  - The microphone is open only while it is wanted, and a failed one falls back to output only.
  - Rebuilds back off from 50 ms to 2 s and give up after 10 s.
  - Desired state is replayed into every fresh engine.
  - 14 tests.
- **FFI:** an opaque `AudioSession` and a `StreamSink` of snapshots. Debug builds install
  `AllocDisabler` as the global allocator. `diapason_ffi` reaches Rust only through `session`,
  which `check-deps` enforces.
- **Dart:**
  - `SessionSnapshot` in `core_domain`.
  - `EngineHandle` gains commands and a snapshot stream.
  - `verifyEngineContract` checks fake and real engine alike.
  - The providers moved down to `audio_engine`.
- **Tuner:** `permission_handler` behind `MicrophonePermission`, and a tuner permission flow with
  widget tests per state.
- **`just test-integration-android`:** the contract on a device through the FFI. The denied half
  passed on the Redmi.
- **The `reviewer` subagent** found a real bug. A device lost for a moment silenced the tuner until
  restart. Fixed with a regression test, confirmed to fail on the old logic, plus five smaller
  fixes.

## Tried and abandoned
- **`permission_handler` 13.** `permission_handler_android` 14 needs compileSdk 37. Gradle
  installed SDK platform 37 into the local SDK on its own before failing. Held at 12.0.3, which
  compiles against 35.
- **Revoking and granting the permission with `pm` for the integration test.** HyperOS refuses it,
  and `adb uninstall` too. The script falls back to the system prompt.
- **`BigInt` for `u64` across FRB.** FRB's `type_64bit_int` gives Dart `int`. There is no web
  target, so nothing is lost.
- **A supervisor in `engine`, `ffi` or `audio_io`.** See `adr/0022`.

## Surprises
- **AAudio refuses to open the input without `RECORD_AUDIO`.** It does not deliver silence, so the
  fallback is reachable on real hardware.
- **The first draft blamed the microphone when both opens failed.** The exact case a route change
  produces.
- **`ARCHITECTURE.md` §4 promised latest-wins all the way to the UI.** Posts to Dart's port queue
  while the isolate stalls. The doc now says so.
- **`ios-project-check` compares with HEAD**, not the index.

## Left for next session
- `just test-integration-android` on the Redmi with the owner at the device (install and microphone
  prompts). Record the refused-microphone error it prints in the task file.
- Then push and PR (owner), then part 2b.

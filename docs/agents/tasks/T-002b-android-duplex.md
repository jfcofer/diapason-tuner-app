---
id: T-002b
title: Android duplex stream on the budget reference device
status: in-progress
milestone: M1
owner: claude
created: 2026-10-04
---

## Goal

Real microphone in and real audio out on Android, through the `T-002a` engine, on the device this
project actually has: the **Redmi 23117RA68G** (Android 16, arm64). It is the "budget Android"
row of `TESTING.md` §7, where the fast path is most likely to be refused, which makes it the most
informative device to start on.

## Context

Read first: `docs/PLATFORM_AUDIO.md` in full. Parent: `T-002`. Depends on `T-002a`.

**Decide the binding first, on evidence, with an ADR.** The two candidates:

- the `oboe` crate: a C++ shim, ships `libc++_shared`, with an OpenSL ES fallback for API 26–27,
  where AAudio is unreliable. Check its maintenance status;
- AAudio directly via the `ndk` crate: pure Rust with no C++, but input presets need API 28+, and
  the pin was `minSdk 26` (`tools/versions.env`); `T-008` raises it to 28.

Measure the same things `T-001a` did: the symbols in the shipped `.so`, size delta, 16 KB
alignment, and that it **links**, not merely compiles. If the evidence says to raise `minSdk`,
that is its own task and its own ADR (`AGENTS.md` §8).

Emulator: fine for the permission and lifecycle rows. **Never** use it for latency or callback
timing; emulated audio says nothing about real devices.

## Acceptance criteria

- [x] ADR for the Android audio binding, with the measurements above
- [x] The Android backend passes the `T-002a` conformance suite (on device, via an integration
      test)
- [ ] Duplex: mic in and output out on one stream and one clock, on the Redmi
- [ ] Input preset per `PLATFORM_AUDIO.md` (Unprocessed if supported, else VoiceRecognition), and
      the preset *actually obtained* is reported
- [ ] `RECORD_AUDIO` in the manifest. The app manifest currently declares **no** permissions
- [ ] A real `MicrophonePermission` in `core_platform` behind the existing interface, covering
      denial and revocation-while-running. The metronome path is unaffected by denial
- [ ] Diagnostics overlay: sample rate, buffer size, round-trip latency, worst-case callback
      duration, xrun count, input preset
- [ ] Android rows of the `PLATFORM_AUDIO.md` §5 lifecycle matrix verified on device; automatable
      rows covered by an integration test
- [ ] Stream rebuilds off the RT thread on route change and device disconnect
- [ ] Round-trip latency on the Redmi recorded in the journal
- [x] The RT clock exception (`clock_gettime` in the callback) has its ADR, and `AGENTS.md` §6
      cites it, so the documented RT rule stays true
- [ ] `just verify` green; `build-android` CI green

## Out of scope

iOS (`T-002c`). Pitch detection. Latency *calibration* UI (M5).

## Implementation notes

**Part 1 (#11, merged 2026-10-09):** binding, backend, on-device conformance (`adr/0020`).
- `ndk-sys` with our own wrapper, not `ndk` (owner's choice): `ndk` 0.9's stream is not `Send`,
  and its Drop unwraps `AAudioStream_close`.
- `audio_io` has no RT `clippy.toml`. The trampoline runs inside `assert_no_alloc`, and a device
  canary proves the trap fires.
- The review's fixes and the device log are in the journal: `2026-10-09-aaudio-review.md` and
  `2026-10-09-aaudio-backend.md`.

**Found on the Redmi:**
- The output is refused the fast path as the shell user, apparently by a vendor per-app policy
  (`UseAAudioApp`). Measure `granted_paths` from the app.
- 9–16 input underruns at start-up, then 0.
- Worst release callback: 281–571 µs of 20 ms.
- Use `adb shell -n` in loops.
- **HyperOS:** the shell may not `pm grant`/`revoke` or `adb uninstall`, and every USB install
  waits for a tap on the device.

**Part 2a (2026-10-09):** `session` crate (`adr/0022`), FFI start/stop and snapshot stream, the
debug-app allocation trap, the permission, the tuner flow, `just test-integration-android`.
- **Deviation:** on denial the tuner opens no stream. A dev-only A4 tone button proves output
  works without the microphone, instead of a tone forced on at denial.
- **`permission_handler` is held at 12.** 13 needs compileSdk 37: its own task (`AGENTS.md` §8).
- **AAudio refuses to open the input without `RECORD_AUDIO`.** It does not deliver silence. The
  session falls back to output only and reports the input fault.

- **The review fixed before the PR:**
  - A device lost for a moment no longer leaves the microphone off for good: both opens failing
    is now blamed on the device.
  - A command dropped on a full queue is re-sent as desired state on the next tick.
  - `stop` clears the input fault.
  - The fake now matches the real engine: rate 0 while stopped, the input fault Android reports.
  - The panel offers "Retry microphone".
  - ARCHITECTURE §4 and §7 match the code.

**Part 3 owes (lifecycle):** Dart never stops the stream yet. Once the tuner listens, the
microphone stays open across screens and in the background until the app exits.

**Part 2b owes:**
- the Kotlin capabilities channel and the preset rule;
- mapping the refused-input error to `PermissionDenied` (today `DeviceUnavailable`);
- the callback budget in release, from the app;
- input-backlog shedding (`getFramesWritten − getFramesRead`), and buffer growth on xruns;
- the shipped-`.so` measurements.

**Plan, approved by the owner on 2026-10-09.** It replaces the binding choice in Context.

- **Binding, duplex shape, configuration, error handling, RT calls:** now decided in `adr/0020`
  (raw `ndk-sys`, not `ndk`; see Part 1 above). minSdk 28 is `adr/0019`.
- **Capabilities** (Unprocessed support, low-latency feature, native rate and burst) come from a
  small Kotlin channel in `core_platform`. Dart passes them to Rust as configuration.
- **Permission:** `permission_handler` behind `MicrophonePermission`. Check its version, licence
  and network behaviour first.
- **On-device conformance:** `just test-android-device` runs the Rust test binary through `adb`.
  Verify first that the shell uid can open an input stream; otherwise use a debug-only FFI entry
  point driven by an `integration_test`.
- **Three PRs:**
  1. the ADR, the backend and on-device conformance;
  2. **split by the owner on 2026-10-09:**
     - **2a:** a new `session` crate (the supervisor that rebuilds streams; `engine`'s RT lint
       config cannot host it), FFI start/stop, the snapshot stream, the debug-app allocation
       trap, and the permission;
     - **2b:** the capabilities channel and preset choice, plus everything "Part 2b also owes"
       lists above;
  3. the overlay (the first golden, which closes the `adr/0016` gap), lifecycle and latency.
- **Criterion amendment:** metronome-column lifecycle rows (FGS, MediaSession, lock screen) need
  the M4 metronome and move there. They are not ticked here.
- **Latency:** the in-app estimate from AAudio timestamps, cross-checked against OboeTester's
  round-trip measurement.

## Verification performed

- **Part 1:** `just test-android-device` on the Redmi 23117RA68G (Android 16, API 36), at the
  committed code:
  - `android_conformance`: 5/5 (duplex, output-only, forced block cutting, microphone keeps up, and
    the device report);
  - `android_alloc_canary`: exit 134 with "memory allocation of 64 bytes failed".

  `--release`: 5/5.
- `just lint-rust`, now including `lint-rust-android`, and `just verify`: green.

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
- [x] Duplex: mic in and output out on one stream and one clock, on the Redmi
- [ ] Input preset per `PLATFORM_AUDIO.md` (Unprocessed if supported, else VoiceRecognition), and
      the preset *actually obtained* is reported
- [x] `RECORD_AUDIO` in the manifest. The app manifest currently declares **no** permissions
- [x] A real `MicrophonePermission` in `core_platform` behind the existing interface, covering
      denial and revocation-while-running. The metronome path is unaffected by denial
- [ ] Diagnostics overlay: sample rate, buffer size, round-trip latency, worst-case callback
      duration, xrun count, input preset
- [ ] Android rows of the `PLATFORM_AUDIO.md` §5 lifecycle matrix verified on device; automatable
      rows covered by an integration test
- [x] Stream rebuilds off the RT thread on route change and device disconnect
- [ ] Round-trip latency on the Redmi recorded in the journal
- [x] The RT clock exception (`clock_gettime` in the callback) has its ADR, and `AGENTS.md` §6
      cites it, so the documented RT rule stays true
- [ ] `just verify` green; `build-android` CI green

## Out of scope

iOS (`T-002c`). Pitch detection. Latency *calibration* UI (M5).

## Implementation notes

**Part 1 (#11, merged 2026-10-09):** binding, backend, on-device conformance (`adr/0020`): raw
`ndk-sys`, since `ndk` 0.9's stream is not `Send`. The callback runs inside `assert_no_alloc`, and
a device canary proves it. The review and device log: journal `2026-10-09-aaudio-*.md`.

**Found on the Redmi:**
- The app gets the low-latency (FAST) path, but not exclusive: its MMAP policy is "never".
- 9–16 input underruns at start-up, then 0. The worst release callback is 281–571 µs of 20 ms.
- HyperOS refuses shell `pm grant`/`revoke` and `adb uninstall`; USB installs wait for a tap. Use
  `adb shell -n` in loops.

**Part 2a (#13, merged 2026-10-09):** `session` crate (`adr/0022`), FFI start/stop and snapshot
stream, the debug-app allocation trap, the permission, the tuner flow, `test-integration-android`.
- **Deviation:** on denial the tuner opens no stream. A dev-only A4 tone button proves output
  works without the microphone, instead of a tone forced on at denial.
- **`permission_handler` is held at 12.** 13 needs compileSdk 37: its own task (`AGENTS.md` §8).
- **AAudio refuses to open the input without `RECORD_AUDIO`,** with `-896`
  (`AAUDIO_ERROR_INTERNAL`). It does not deliver silence. The session falls back to output only
  and reports the input fault. The code is generic, so 2b cannot map it to `PermissionDenied` by
  code alone. Use the permission status Dart already holds.
- The review's fixes before #13 are in the journal (`2026-10-09-session-supervisor.md`).

**Part 3 owes (lifecycle):** Dart never stops the stream yet. Once the tuner listens, the
microphone stays open across screens and in the background until the app exits. In the
background the platform silences it (exact zeros, no error), so the tuner hears nothing without
knowing why.

**Part 2b-i (2026-10-09), capabilities** (`adr/0024`):
- a Pigeon channel in `core_platform` (now a plugin), and `choose_input_preset` in Rust;
- the permission told to the session (`MicrophoneAccess`): a denied microphone is never opened,
  and is reported as `PermissionDenied`;
- the preset requested reported beside the one obtained; the dev A4 button really dev-only.

**Deviations:** only the two facts the rule reads cross to Rust (AAudio picks rate and burst), and
Pigeon's output is generated by `just deps`, not committed.

**Review (before the device run):** the denied half of the contract would have timed out on the
Redmi. A denial reported before the microphone was wanted was never stored. The fault is now
derived; a new grant retries; a repeated grant does not. A driver test replays the contract's order
on the host, and was shown to fail without the fix.

**Still owed by 2b-i:** proof the preset is applied. `AudioRecord` logs `inputSource 0` although
`VOICE_RECOGNITION` was asked, and "obtained" only echoes the request. Run
`adb shell -n dumpsys audio | grep -iA4 'recording activity'` while the tuner listens.

**Part 2b-ii owes, stream tuning:** falling back to VoiceRecognition when an Unprocessed input
fails to open (review), input-backlog shedding (`getFramesWritten − getFramesRead`), buffer
  growth on xruns, the callback budget in release from the app, and the shipped-`.so`
  measurements as a `check-android-release` gate.

**Plan, approved by the owner on 2026-10-09.** It replaces the binding choice in Context.

- **Decided:** the binding in `adr/0020`, minSdk 28 in `adr/0019`, the session in `adr/0022`.
- **Capabilities** (Unprocessed support, low-latency feature, native rate and burst) come from a
  small Kotlin channel in `core_platform`. Dart passes them to Rust as configuration.
- **PRs:** 1, 2a, 2b-i and 2b-ii, as above. Then 3: the overlay (the first golden, which closes
  the `adr/0016` gap), lifecycle and latency.
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
- **Part 2b-i, Redmi** (`test-integration-android`, owner, 2026-10-10): the device reports
  unprocessed **false** and low latency **true** (native 48 kHz / 256 frames), so VoiceRecognition
  is chosen. Denied: `permissionDenied`, with the output running. Granted: duplex, VoiceRecognition
  requested and obtained, low-latency input, 48 kHz, burst 960. The `dumpsys` was taken after the
  stream closed, so it holds no recording client: the preset proof is still owed.
- **Part 2b-i, host:** `just verify` green; `session` 21 tests, including the preset truth table and
  five permission transitions, each with desired state replayed.
- **Part 2a:** `just test-integration-android` on the Redmi, run by the owner. Both halves passed
  with the debug allocation trap armed:
  - **without the microphone:** the output ran, the input fault was reported, and the input stream
    was refused with `-896`;
  - **with it, allowed at the system prompt:** duplex.
- **By hand, on the Redmi (owner, logcat kept):**
  - each wired-headphone plug and unplug disconnected the stream, and the session rebuilt it on
    the new device, with no crash;
  - revoking the microphone in Settings killed the running app.

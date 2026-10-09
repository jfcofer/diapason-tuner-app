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

**Part 1 of 3, done 2026-10-09:** the binding, the backend and on-device conformance.
`adr/0020` has the full evidence. Deviations from the plan below:

- **`ndk-sys` with our own wrapper, not `ndk`.** The owner chose this after `ndk` 0.9's source
  showed `AudioStream` is not `Send` and its Drop unwraps `AAudioStream_close` (an abort under
  `panic = "abort"`).
- **No `clippy.toml` symlink for `audio_io`.** Its device tests must sleep and read the clock, and
  clippy cannot scope the list per target. Instead the trampoline runs inside `assert_no_alloc`,
  and `android_alloc_canary` proves on device that the trap fires.
- **The ADR's shipped-`.so` figures wait for part 2.** Nothing calls the backend yet, so the linker
  strips it. Linking is proven on the device test binary instead (only `libaaudio`, `libdl`,
  `libc`).

**Found on the Redmi, for part 2:**

- **The output is refused the fast path** as the shell user: AudioFlinger's "mismatch between
  requested flags (00000104) and output flags (00000002)". `USAGE_GAME` makes no difference. The
  log points at a per-app vendor policy (`UseAAudioApp`). **Measure `granted_paths` from the
  app.**
- **Input underruns:** 9–16 at start-up, then 0, which a device test now asserts. The engine should
  treat a rising `input_underruns` as "input warming up" and not analyse those blocks.
- **Worst callback, release, excluding the draining first callback:** 281–571 µs of 20 ms.
- **cargo-ndk 4 ships `cargo-ndk-runner`,** which pushes and runs the exact test binary and returns
  its exit status. `test-android-device.sh` uses it. Use `adb shell -n` in any loop: plain
  `adb shell` reads stdin.

**The review of part 1** (the `reviewer` subagent) was fixed before merge:
- A lost microphone now sets `disconnected`: a failed read, or an error callback, which is now on
  the input too.
- `host_time_ns` is presentation time from `getTimestamp`, as `AUDIO_ENGINE.md` §6 requires. It had
  been render time.
- The first callback's drain is excluded from the worst case.
- A failed output close leaks the state instead of freeing it.
- `Running` is `Send`.
- The seqlock reader yields.
- The device script uses cargo's own runner, and the canary is checked for the allocator's
  message.
- The ADR, `AGENTS.md` §6 and `AUDIO_ENGINE.md` §7 no longer overclaim. The trap sees only Rust's
  allocator, and `read`/`getTimestamp` may take a platform mutex on the legacy path.

**Part 2b also owes:**
- the §1 callback budget measured in release, from the app;
- shedding an input backlog that builds up after start-up (use `getFramesWritten −
  getFramesRead`), since it is otherwise permanent latency;
- growing the buffer when xruns rise;
- mapping a denied microphone to `AudioError::PermissionDenied`, which today surfaces as
  `Platform`;
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

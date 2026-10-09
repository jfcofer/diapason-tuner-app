# 0020 — Android audio: AAudio through raw `ndk-sys`, with our own wrapper

**Status:** Accepted · 2026-10-09

## Context

`T-002b` needs one duplex stream on Android, with an input preset, that every backend contract
check in `audio_io::conformance` passes. minSdk is 28 (`adr/0019`), so the whole AAudio API,
input presets included, is available. Three Rust bindings were candidates.

- **`oboe` 0.6.1.** No release or commit since 2024-03-03. It ships `libc++_shared`.
- **`ndk` 0.9.0, the safe wrapper.** Maintained, and cpal 0.18 uses it. Reading its source found
  two defects that matter here:
  - `AudioStream` is not `Send`, and its raw pointer is private. Duplex reads the input stream from
    inside the output stream's callback, which must be `Send`.
  - `Drop for AudioStream` calls `.unwrap()` on `AAudioStream_close`. Our release profile is
    `panic = "abort"`, so a close error during teardown would kill the app, and teardown after a
    device disconnect is exactly when close is most likely to fail.

  The upstream fixes were closed unmerged: #496 (`Sync`) and #497 (`Send`), the latter on
  2026-07-21.
- **`ndk-sys` 0.6.0.** The bindgen output beneath `ndk`, with all 67 AAudio functions. Its only
  dependency is `jni-sys`.

## Decision

- **`audio_io::AAudioBackend` uses `ndk-sys` directly, through a wrapper written for this
  contract** (`rust/crates/audio_io/src/android.rs`):
  - Builders and streams are RAII types, and close errors are returned as `Result`.
  - `unsafe impl Send` is justified once, against AAudio's documented thread-safety rules.
  - Every `unsafe` block carries a `// SAFETY:` comment, enforced by
    `clippy::undocumented_unsafe_blocks = "deny"` on the crate.
- **Duplex is two AAudio streams on one clock.** AAudio's own round-trip example uses the same
  pattern, as does Oboe's `FullDuplexStream`.
  - The output stream's data callback drives everything, and its frame count is the stream clock.
  - It reads the input stream with a zero timeout into a buffer preallocated at open.
  - Short reads are padded with silence and counted in `StreamHandle::input_underruns`.
  - On the first callback the input backlog is drained, a bounded number of reads.
- **Configuration:**
  - float samples, low-latency performance mode, exclusive sharing (AAudio falls back to shared by
    itself);
  - the device's native rate, with the requested rate ignored. The input asks for the rate the
    output was granted;
  - `USAGE_MEDIA` / `CONTENT_TYPE_MUSIC`;
  - a buffer of two bursts;
  - the input preset as requested, then read back. `obtained_input_preset` and `granted_paths`
    report what the device actually gave.
- **Shutdown order is the safety argument.** The output is stopped, `waitForStateChange` confirms
  it, and it is closed. Only then is the input stopped and closed, and the callback state freed. No
  callback can be running against anything that has been freed.
- **The error callback only sets a flag** (`StreamHandle::disconnected`). AAudio forbids stopping
  or reopening from that thread, so rebuilding is the engine's job, on a normal thread (`T-002b`,
  part 2).
- **The one timing call on the real-time path is `clock_gettime(CLOCK_MONOTONIC)`.** It provides
  `host_time_ns` and the worst-case callback duration. It is served by the vDSO on the 64-bit ABIs,
  so it reads shared memory and makes no kernel entry. `AGENTS.md` §6 cites this ADR for it. On the
  32-bit `armeabi-v7a` it may enter the kernel; that ABI is a small minority of minSdk-28 devices.
- **The real-time rules are enforced on the device, not by the shared clippy list:**
  - the trampoline runs every callback inside `assert_no_alloc`;
  - the device tests install `AllocDisabler` as the global allocator, and `android_alloc_canary`
    proves the trap fires (SIGABRT).

  `audio_io` does not take `engine/clippy.toml`. Most of the crate is control-side, its device tests
  must sleep and read the clock, and clippy cannot scope the list per target.
- **Android-only code is linted by `just lint-rust-android`.** The host's `lint-rust` never
  compiles it.

## Measurements (Redmi 23117RA68G, Android 16, `just test-android-device`, 2026-10-09)

- **Conformance:** `run_all` passes for duplex, output-only and forced block cutting
  (`max_block_frames` 32 against a 960-frame burst). The canary aborts.
- **Linking** (the device test binary): it needs only `libaaudio.so`, `libdl.so` and `libc.so`,
  with **no `libc++_shared`**. It imports 29 AAudio symbols, among them the API-28
  `setInputPreset`, `setUsage` and `setContentType`. `LOAD` alignment is `0x4000`. `cargo deny`
  is clean.
- **Shipped `.so`:** no size delta and no AAudio imports yet. Nothing in the FFI calls the backend
  until part 2 wires the engine to it, and the linker strips it. Part 2 records both figures.
- **Granted:**
  - 48 kHz with a 960-frame burst (20 ms). Both `Unprocessed` and `VoiceRecognition` were obtained
    as requested.
  - Worst callback 2.3–3.0 ms; xruns 0.
  - Input underruns 10–16 in the first 2 s, then 0 over the next 3 s. They are start-up, not drift.
- **The output was refused the fast path as the shell user.** AudioFlinger logs
  `createTrack_l(): mismatch between requested flags (00000104) and output flags (00000002)`: the
  policy routes the track to the primary output, which has no FastMixer, although the device has a
  FastMixer output (256-frame HAL) and advertises `android.hardware.audio.low_latency`.
  - `USAGE_GAME` changes nothing.
  - The log's `UseAAudioApp mClientName :com.android.shell()` → `final mmap policy is 1` points at
    a per-app vendor policy.
  - Whether the real app gets the fast path is measured from the app in part 2.

## Consequences

**Good.**
- No third-party code between the engine and AAudio that can panic on teardown.
- No C++ runtime in the APK.
- Every rule the contract states is checked on hardware, and the trap that proves "no allocation"
  is itself proven.

**Bad.**
- About 600 lines of `unsafe`-heavy wrapper are ours to maintain. AAudio's API is stable and
  frozen since API 30, which bounds that cost.
- There is no OpenSL ES fallback. minSdk 28 (`adr/0019`) makes one unnecessary.
- Device conformance needs a device. CI cannot run it, so each session that touches `android.rs`
  runs `just test-android-device`.

**Rejected.** `oboe` (unmaintained, `libc++_shared`). The `ndk` wrapper (not `Send`, and its Drop
can abort). Forking `ndk` to fix both, which would mean maintaining a fork of a large crate for 2%
of its surface.

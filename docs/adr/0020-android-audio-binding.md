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
    `panic = "abort"`, so a close error during teardown would kill the app.

  The upstream fixes were closed unmerged: #496 (`Sync`) and #497 (`Send`), the latter on
  2026-07-21.
- **`ndk-sys` 0.6.0.** The bindgen output beneath `ndk`, with all 67 AAudio functions. Its only
  dependency is `jni-sys`.

## Decision

- **`audio_io::AAudioBackend` uses `ndk-sys` directly, through a wrapper written for this
  contract** (`rust/crates/audio_io/src/android.rs`):
  - Builders and streams are RAII types, and close errors are returned as `Result`.
  - `Send` for `Stream` and `Running` is justified against AAudio's thread-safety rules: `get*` is
    thread-safe except `getTimestamp`, and read and close have one owner each.
  - `clippy::undocumented_unsafe_blocks = "deny"` enforces a `// SAFETY:` comment on every
    `unsafe` block.
- **Duplex is two AAudio streams on one clock.** AAudio's own round-trip example uses the same
  pattern, as does Oboe's `FullDuplexStream`.
  - The output stream's data callback drives everything, and its frame count is the stream clock.
  - It reads the input with a zero timeout into a buffer preallocated at open.
  - Short reads are padded with silence and counted (`input_underruns`).
  - A failed read sets `disconnected`, because the input stream has no other reliable way to
    report a lost microphone.
  - The first callback drains the input backlog, a bounded number of reads.
- **Configuration:** float, low latency, exclusive (AAudio falls back to shared itself), the
  native rate (the input asks for the output's), `USAGE_MEDIA`/`CONTENT_TYPE_MUSIC`, two bursts.
  The preset and the granted modes are read back (`obtained_input_preset`, `granted_paths`).
- **`host_time_ns` is presentation time** (`AUDIO_ENGINE.md` §6). Each callback asks the output
  stream for `getTimestamp(CLOCK_MONOTONIC)` on the callback thread, the only thread allowed to,
  and extrapolates each block's first frame from it. Until the device reports a presented frame,
  render time stands in. The time is kept strictly rising.
- **Shutdown order is the safety argument.** The output is stopped, `waitForStateChange` confirms
  it, and it is closed. Only then is the input stopped and closed, and the callback state freed.
  If closing the output fails, the state is leaked rather than freed: a stream that would not
  close may still call back.
- **Error callbacks (on both streams) only set a flag.** AAudio forbids stopping or reopening from
  them, so rebuilding is the engine's job, on a normal thread (`T-002b` part 2).
- **The RT path makes three audited platform calls** (`AGENTS.md` §6, `AUDIO_ENGINE.md` §7):
  - **`AAudioStream_read`.** On the legacy, non-MMAP capture path (this Redmi's), it takes
    `AudioRecord`'s mutex. That is the cost Oboe accepts too. An MMAP stream would not take it.
  - **`AAudioStream_getTimestamp`.** The same applies on the legacy output path.
  - **`clock_gettime(CLOCK_MONOTONIC)`,** for the fallback stamp and the callback duration. It is
    served by the vDSO on the 64-bit ABIs. On 32-bit `armeabi-v7a`, which the app bundle still
    ships, it may enter the kernel.
- **Rust heap allocation on the RT path is trapped at run time, not linted.**
  - The trampoline runs every callback inside `assert_no_alloc`.
  - The device tests make `AllocDisabler` the global allocator, and `android_alloc_canary` proves
    the trap fires (SIGABRT, with the allocator's message).
  - It cannot see allocations inside `libaaudio` or `libaudioclient`.

  `audio_io` does not take `engine/clippy.toml`. Most of the crate is control-side, its device
  tests must sleep and read the clock, and clippy cannot scope the list per target.
- **Android-only code is type-checked by `just lint-rust-android`** without an NDK. It is part of
  `lint-rust`, so CI checks it.

## Measurements

Redmi 23117RA68G, Android 16, `just test-android-device`, at the committed code, 2026-10-09.

- **Conformance:** `run_all` passes for duplex, output-only and forced block cutting
  (`max_block_frames` 32 against a 960-frame burst). After warm-up, no block is short of input.
  The canary aborts.
- **Linking** (the device test binary): only `libaaudio`, `libdl` and `libc`, **no
  `libc++_shared`**. 29 AAudio imports, including the API-28 `setInputPreset`. `LOAD` alignment
  `0x4000`. `cargo deny` is clean.
- **Shipped `.so`:** no size delta and no AAudio imports yet. Nothing in the FFI calls the backend
  until part 2 wires the engine to it, and the linker strips it. Part 2 records both figures.
- **Granted:** 48 kHz, a 960-frame burst (20 ms), both presets as requested, xruns 0. Input
  underruns: 9–16 in the first 2 s, then 0 over the next 3 s, so start-up, not drift.
- **Worst callback, excluding the first (draining) callback:**
  - release: **281–571 µs** over two runs, at most 3% of the 20 ms period, against
    `AUDIO_ENGINE.md` §1's ≤15%;
  - debug: 581–653 µs.

  Including the drain, debug reached 2.3–5.9 ms. That is why the first callback is left out.
- **The output was refused the fast path as the shell user.** AudioFlinger logs
  `createTrack_l(): mismatch between requested flags (00000104) and output flags (00000002)`: the
  policy routes the track to the primary output, which has no FastMixer, although the device has a
  FastMixer output (256-frame HAL) and advertises `android.hardware.audio.low_latency`.
  - `USAGE_GAME` changes nothing.
  - `UseAAudioApp mClientName :com.android.shell()` → `final mmap policy is 1` points at a
    per-app vendor policy.
  - Whether the app gets the fast path is measured from the app in part 2.

## Consequences

**Good.**
- No third-party code between the engine and AAudio that can panic on teardown.
- No C++ runtime in the APK.
- The contract, input delivery and the absence of Rust allocation on the audio thread are checked
  on hardware, and the trap that proves the last is itself proven.

**Bad.**
- About 900 lines of `unsafe`-heavy wrapper are ours to maintain. AAudio still gains functions
  (API 31–36), but none that this backend needs.
- There is no OpenSL ES fallback. minSdk 28 (`adr/0019`) makes one unnecessary.
- Device conformance needs a device. CI cannot run it, so each session that touches `android.rs`
  runs `just test-android-device`.
- Left for part 2: shedding an input backlog that builds up after start-up, growing the buffer on
  xruns, and mapping a denied microphone to `AudioError::PermissionDenied`.

**Rejected.** `oboe` (unmaintained, `libc++_shared`). The `ndk` wrapper (not `Send`; its Drop can
abort). Forking `ndk`: a large crate to maintain for 2% of its surface.

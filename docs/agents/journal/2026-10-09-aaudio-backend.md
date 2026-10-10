# 2026-10-09 — aaudio-backend

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b (part 1 of 3)  **Milestone:** M1

## Done
- `AAudioBackend`, on raw `ndk-sys` with our own RAII wrapper (`adr/0020`), passes the conformance
  suite on the Redmi for duplex, output-only and forced block cutting.
- `android_alloc_canary` aborts, proving the on-device allocation trap is armed.
- `just test-android-device` runs both over adb. `lint-rust-android` type-checks the Android code
  without an NDK and is part of `lint-rust`, so CI lints it.
- `StreamHandle` gained worst-callback, input-underrun and disconnected readings.
  `AudioError::Platform` carries AAudio's codes without allocating.

## Tried and abandoned
- **The `ndk` safe wrapper**, the approved plan. Its `AudioStream` is not `Send` (the private
  pointer rules out a clean newtype), and its Drop unwraps `AAudioStream_close`, which is an
  abort under `panic = "abort"`. The upstream fixes, #496 and #497, were closed unmerged. The
  owner chose `ndk-sys`.
- **Symlinking the RT `clippy.toml` into `audio_io`.** It would ban the sleep and clock the device
  harness needs, crate-wide. It was replaced by running `assert_no_alloc` around the trampoline,
  plus a canary.
- **`USAGE_GAME` to get the output fast path.** Same refusal as `USAGE_MEDIA`, so it was reverted.

## Surprises
- **The Redmi refuses the output fast path to the shell user,** although it has a FastMixer and
  `audio.low_latency`. AudioFlinger: "mismatch between requested flags (00000104) and output
  flags (00000002)", and `UseAAudioApp … final mmap policy is 1`. A vendor per-app policy is
  suspected; it has to be measured from the app.
- **Two conformance checks raced on real hardware:** the post-open `timestamp() == None`, and the
  handle's largest block against the probe's. Both are now relaxed under `REALTIME`, with the
  reason given.
- **My own first draft had bugs, caught before any device run:**
  - the stop wait could return while the stream was still `STARTED`;
  - the input was closed unstopped;
  - `AAUDIO_FORMAT_PCM_FLOAT` is `c_int`, not `c_uint`.
- **cargo-ndk 4.1.2 drops `--message-format=json`, and `adb shell` eats loop stdin.** Both are
  recorded in the task.
- **Input underruns happen only at start-up** (10–16 in 2 s, then 0).

## Left for next session
- T-002b part 2:
  - engine session and supervisor;
  - FFI start/stop and the ~30 Hz snapshot stream;
  - `MicrophonePermission`, capabilities, `RECORD_AUDIO`;
  - the debug FFI's `AllocDisabler`;
  - shipped-`.so` measurements;
  - `granted_paths` read from the app.

# 2026-10-10 — device-run

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2b-i  **Milestone:** M1

## Done
- **The owner ran `just test-integration-android` on the Redmi.** Both halves passed.
  - Capabilities: unprocessed **false**, low latency **true**, native 48 kHz / 256 frames, so
    VoiceRecognition was chosen.
  - Denied: `permissionDenied`, with the output running.
  - Granted: duplex, VoiceRecognition requested and obtained, a low-latency (not exclusive) input,
    48 kHz, burst 960.
- **The preset is proven applied.** During `just run android`, the owner's
  `dumpsys media.audio_policy` showed the capture client's attributes with
  `Source: AUDIO_SOURCE_VOICE_RECOGNITION` (`Source: 6`). The criterion is ticked. Caveat: the
  grep excerpt omits the client's uid, but it was the only capture client while Diapason
  (uid 10381) ran.

## Tried and abandoned
- **Capturing the preset proof by launching the app over `adb`.** WhatsApp was in the foreground:
  the owner was using the phone. The agent stopped without tapping anything, and deleted the UI
  dump it had written to `/sdcard`. On a personal device, ask first.

## Surprises
- **The first `media.audio_policy` dump held no capture client**, because it was taken after the
  test had closed the stream. The proof needs a live microphone. With one, the input's attributes
  carry the real source.
- **`AudioRecord` logs `inputSource 0` while applying VOICE_RECOGNITION.** The log prints `set()`'s
  first argument, and AAudio passes the source in the attributes instead.
- **The debug integration build compiles Rust for three ABIs** (`aarch64`, `i686`, `x86_64`),
  although one device is attached. Worth a look in 2b-ii, where build cost matters on this host.

## Left for next session
- Push and open the 2b-i PR, when the owner says so. Then 2b-ii.

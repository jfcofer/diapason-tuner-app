# 2026-10-10 — device-run

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2b-i  **Milestone:** M1

## Done
- **The owner ran `just test-integration-android` on the Redmi.** Both halves passed.
  - Capabilities: unprocessed **false**, low latency **true**, native 48 kHz / 256 frames, so
    VoiceRecognition was chosen.
  - Denied: `permissionDenied`, with the output running.
  - Granted: duplex, VoiceRecognition requested and obtained, a low-latency (not exclusive) input,
    48 kHz, burst 960.
- Recorded in T-002b. Its PR is next.

## Tried and abandoned
- **Capturing the preset proof by launching the app over `adb`.** WhatsApp was in the foreground:
  the owner was using the phone. The agent stopped without tapping anything, and deleted the UI
  dump it had written to `/sdcard`. On a personal device, ask first.

## Surprises
- **The `media.audio_policy` dump held no recording client.** It was taken after the test had
  closed the stream, and that dump lists output tracks, not record clients. So the preset proof is
  still open. Next time run `dumpsys audio` ("Recording activity") while the tuner listens.
- **The debug integration build compiles Rust for three ABIs** (`aarch64`, `i686`, `x86_64`),
  although one device is attached. Worth a look in 2b-ii, where build cost matters on this host.

## Left for next session
- Push and open the 2b-i PR (the owner approves the push).
- The preset proof while the tuner listens; then 2b-ii.

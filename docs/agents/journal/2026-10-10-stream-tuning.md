# 2026-10-10 — stream-tuning

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2b-ii  **Milestone:** M1

## Done
- The owner merged #16 (2b-i).
- **2b-ii** (`adr/0025`):
  - **Backlog shedding**, every callback, from `getFramesWritten − getFramesRead`. It replaces the
    first-callback drain.
  - **Buffer growth** as session policy: one burst per output underrun, up to capacity. Tested on
    the host with `Flaky`.
  - **A refused Unprocessed preset falls back to VoiceRecognition.**
  - **`callbackLoad`**, the worst callback over one burst, with dev readout lines.
  - **The shipped-`.so` gate** in `check-android-release`.
- **Two device tests**: a stalled callback must produce shed frames, and a buffer resize must be
  granted within capacity. Plus `just run … --profile` for the release-mode callback reading.

## Tried and abandoned
- **Growing the buffer inside the backend.** The policy would then be untestable off-device. The
  backend now offers only `set_buffer_frames`; the supervisor decides.
- **Growing on the total xrun count.** An input overrun is not fixed by a larger output buffer,
  so the report now carries `output_xruns` separately.

## Surprises
- **AAudio.h says it plainly:** a data callback may call `AAudioStream_get*()`, and nothing that
  sets. That decided which thread does what.
- **The only local release APK predates the backend wiring** (built 14:04 on 2026-10-09; the
  session crate dates from 18:36). Its `.so` links no AAudio, as `adr/0020` predicted, and the new
  gate fails it. Today's debug `.so` links `libaaudio` with 29 AAudio imports. CI's prod build is
  the gate's first real pass.

## Review
- **No RT-safety bug.** But shedding's slack would have kept ~20 ms of standing input latency on
  the Redmi (one 960-frame burst). Now:
  - it sheds to the read plus at most 5 ms, and only past one burst of delivery jitter;
  - a zero input burst falls back to the output's;
  - start-up shedding is excluded from the worst callback until a callback sheds nothing.
- **The device tests could pass without proving anything.** The stall now fires on the stream
  clock; steady state must shed nothing; a resize must grow where capacity allows.
- **The two getters joined the audited RT calls** (`AUDIO_ENGINE.md` §7), and `adr/0020` now says
  0025 amends it.

## Left for next session
- The owner: `just test-android-device` and `just test-android-device --release` (shell binaries:
  no install taps), then `just run android dev --profile` and read the "Callback:" line.
- Then the PR for 2b-ii; then part 3.

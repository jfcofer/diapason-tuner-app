# 0025 — Shed the input backlog on the audio thread; grow the output buffer as session policy

**Status:** Accepted · 2026-10-10 · amends [0020](0020-android-audio-binding.md) (the first-callback
drain, and the list of audited RT calls)

## Context

`adr/0020` left three things for `T-002b` part 2: shedding an input backlog that builds up after
start-up, growing the buffer on xruns, and a fallback when a preset is refused. Each is a choice
about where code runs, and two of them touch the audio thread.

- **Backlog.** On a duplex stream built from two AAudio streams, the output callback reads the
  microphone with a zero-timeout read. A start-up gap, or any stall, leaves frames queued behind
  the reader. Until now only the first callback drained them. A stall later on became permanent
  latency between the instrument and the tuner.
- **Underruns.** The output buffer was fixed at two bursts. A device that needs more just kept
  underrunning: audible clicks in the metronome, and a gap in the tone.
- **AAudio's rules** (`AAudio.h`, NDK r30): a data callback may call `AAudioStream_get*()`, but
  not `read` or `write` on its own stream, and not anything that can block.
  `setBufferSizeInFrames` is not a `get*` call.

## Decision

- **Start-up keeps `adr/0020`'s drain:** the first callback reads until a read comes up short,
  which is proven on the Redmi. That callback stays out of the worst duration.
- **After that, a backlog is measured against a floor, not zero**, every callback, on the audio
  thread.
  - The gap is `getFramesWritten − getFramesRead` on the input. Both are `get*` calls, which the
    callback may make. **They join the audited RT calls** in `AUDIO_ENGINE.md` §7.
  - The floor is the lowest gap over the last one to two windows of 64 callbacks: two running
    minimums, with no memory.
  - It sheds only when the gap stands more than one input burst plus the slack above the floor,
    and then down to the slack. A burst is how unevenly the microphone delivers. The slack is the
    smaller of one burst and 5 ms (240 frames at 48 kHz): standing latency, so it is kept small.
  - An input burst reported as 0 is replaced by the output's burst.
  - The discarding uses non-blocking reads into the buffer allocated at open, at most 32 of them,
    and stops at the first short read. `StreamHandle::input_frames_shed` counts what it discards.
  - The arithmetic (`audio_io/src/backlog.rs`) compiles for host tests.
  - **Why a floor** (Redmi, 2026-10-10): the first version took the raw gap as backlog. On the
    legacy capture path the counters lead what a read can return, so it shed ~10,000 frames a
    second (~20 % of the microphone) with nothing stalling, and its steady-state test failed. Input
    underruns rose from 0 to ~28 a second.
- **Buffer growth is session policy, on the session thread.**
  - The backend only reports `output_xruns`, `buffer_frames` and `buffer_capacity_frames`, and
    offers `AudioBackend::set_buffer_frames`. The default implementation is a no-op.
  - On each tick, the supervisor grows the buffer by one burst, up to capacity, if output xruns rose
    since the last tick.
  - Input overruns do not grow it: shedding answers those.
  - Every new stream starts again at two bursts.
  - Keeping the policy in `session` means `Flaky` tests it on the host; only the setter needs a
    device.
- **A refused preset falls back once.** If the input fails to open with Unprocessed, it is reopened
  with VoiceRecognition (`InputPreset::fallback`). A disconnect is not retried there; the session
  rebuilds the stream instead.
  - The report keeps the first choice as `requested_input_preset`, so the fallback shows as a
    mismatch with the preset obtained.
  - If the fallback fails too, the first refusal is reported.
- **The callback budget is the worst callback over one burst's duration**, shown as
  `SessionSnapshot.callbackLoad`. `AUDIO_ENGINE.md` §1 sets the budget at 15 %, failing above 40 %.

## Consequences

**Good.**
- After a stall, the stalled-for audio is discarded, and the input latency returns to at most the
  slack plus the device's delivery granularity. A constant counter offset is never read as backlog.
- An underrunning device converges on the smallest buffer that holds.
- The growth policy, the fallback rule and the shedding arithmetic are tested on any host. On the
  device, a stalled callback must be shed, and steady state must shed nothing.

**Bad.**
- **Two getter calls on every callback.** On the legacy capture path they may take the same
  `AudioRecord` mutex as `read` (from memory of AOSP, not verified here): more of the same risk
  `adr/0020` accepted, not a new kind.
- **Up to 5 ms of input latency is kept on purpose,** as a cushion against read timing. The device
  run decides whether it holds: `the_microphone_keeps_up_once_warmed_up` must see neither
  underruns nor shedding in steady state.
- **The supervisor reads the full report on every tick** for the growth check, as `snapshot`
  already did, so the getters run twice per ~33 ms.
- **A lasting rise in the offset** (a route change) reads as backlog for up to two windows, so it
  may be shed once. Accepted: the stream is usually rebuilt on a route change anyway.
- Growth never shrinks back within a stream, so one bad moment raises latency until the next
  rebuild. Oboe's tuner behaves the same way. Shrinking is left until a device shows it is needed.
- The fallback path cannot be exercised on the one device this project has: the Redmi opens
  Unprocessed.

**Rejected.**
- **Growing the buffer in the callback**, as Oboe's `LatencyTuner` can: `setBufferSizeInFrames` is
  not on AAudio's list of callback-safe calls.
- **Draining the whole backlog every callback:** a device delivering faster than the reads would
  hold the audio thread, and an empty queue underruns. That is why reads are bounded and keep a
  slack.
- **The raw counter gap as the backlog:** disproved on the Redmi (see *Why a floor*).

# 0025 — Grow the output buffer as session policy; discard input only at start-up

**Status:** Accepted · 2026-10-21

## Context

`adr/0020` left three things for `T-002b` part 2: shedding an input backlog that builds up after
start-up, growing the buffer on xruns, and a fallback when a preset is refused. Each is a choice
about where code runs, and two of them touch the audio thread.

- **Backlog.** On a duplex stream built from two AAudio streams, the output callback reads the
  microphone with a zero-timeout read. The first callback drains what queued up before the speaker
  started (`adr/0020`). A stall later on would, in principle, leave its audio queued as latency.
- **Underruns.** The output buffer was fixed at two bursts. A device that needs more just kept
  underrunning: audible clicks in the metronome, and a gap in the tone.
- **AAudio's rules** (`AAudio.h`, NDK r30): a data callback may call `AAudioStream_get*()`, but
  not `read` or `write` on its own stream, and not anything that can block.
  `setBufferSizeInFrames` is not a `get*` call.

## Decision

- **Input is discarded only by `adr/0020`'s start-up drain.** No callback after the first discards
  microphone frames. `StreamHandle::input_frames_shed` counts what the drain discards.
- **Buffer growth is session policy, on the session thread.**
  - The backend only reports `output_xruns`, `buffer_frames` and `buffer_capacity_frames`, and
    offers `AudioBackend::set_buffer_frames`. The default implementation is a no-op.
  - On each tick, the supervisor grows the buffer by one burst, up to capacity, if output xruns rose
    since the last tick. Input overruns do not grow it: a larger output buffer does not fix them.
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
- The callback's input path is exactly `adr/0020`'s, which the Redmi runs with 0 input underruns
  after warm-up, in debug and release. No new RT calls.
- An underrunning device converges on the smallest buffer that holds.
- The growth policy and the fallback rule are tested on any host; the setter on the device.

**Bad.**
- **A stall's audio stays queued as latency until the stream is rebuilt.** Nothing on the Redmi
  has shown such a stall yet. Recovering from one is left to a later slice, which measures a real
  stall on the device before choosing a mechanism.
- **The supervisor reads the full report on every tick** for the growth check, as `snapshot`
  already did.
- Growth never shrinks back within a stream, so one bad moment raises latency until the next
  rebuild. Oboe's tuner behaves the same way. Shrinking is left until a device shows it is needed.
- The fallback path cannot be exercised on the one device this project has, which opens
  Unprocessed when the shell user asks for it.

**Rejected.**
- **Growing the buffer in the callback**, as Oboe's `LatencyTuner` can: `setBufferSizeInFrames` is
  not on AAudio's list of callback-safe calls.
- **Shedding a backlog every callback, measured from the input's counters**
  (`getFramesWritten − getFramesRead`). It was built and measured on the Redmi, and failed twice:
  - taking the raw gap as backlog shed ~10,000 frames a second with nothing stalling (2026-10-10);
  - measuring it from the gap's running minimum shed ~34,000 of 48,000 frames a second, and input
    underruns rose from 0 to ~70 a second (2026-10-21). On the same day and device, with the same
    granted paths, `adr/0020`'s code had 0 after warm-up.

  Sampled from outside the callback, the gap stayed within 0–960 frames (one burst), so the
  counters did not show a queue the reads could drain. A review also found the floor would absorb
  any rise below its threshold, letting latency build up silently. Journals
  `2026-10-10-stream-tuning.md` and `2026-10-21-close-stream-tuning.md`.
- **Draining the whole backlog every callback:** a device delivering faster than the reads would
  hold the audio thread, and an empty queue underruns.

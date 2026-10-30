---
id: T-002f
title: Estimate round-trip latency on the device, and check it against OboeTester
status: todo
milestone: M1
owner: unassigned
created: 2026-10-20
---

## Goal

M1's exit asks for round-trip latency measured and reported on every reference device. The metronome
(M4/M5) and latency calibration (M5) both stand on this number, and on the Redmi the output was
seen refused the fast path on 2026-10-10. Produce an in-app estimate the session reports, prove it
against an independent measurement, and find out why the output lost the fast path.

## Context

Read: `docs/PLATFORM_AUDIO.md` (latency sections), `docs/AUDIO_ENGINE.md` §7 (audited RT calls),
`docs/adr/0020`, `docs/adr/0025`. A slice of `T-002`, split out of `T-002b` part 3. The
`dsp-engineer` agent's scope for `engine`/`session`.

- The plan approved on 2026-10-09: the estimate comes from AAudio timestamps
  (`AAudioStream_getTimestamp` on each direction, with frame positions), computed **off** the RT
  thread in `session`, and is cross-checked against OboeTester's round-trip test on the same device
  and route. `getTimestamp` is not on the audited list of callback calls; keep it off the callback,
  or audit it (`AGENTS.md` §6).
- **Seen on 2026-10-10** (journal `2026-10-10-stream-tuning.md`): the output requested
  low-latency and was given `flags 0x104 → 0x0`, perfMode 12 → 10, although part 1 had it granted.
  Find which change caused it (preset, buffer resize, sharing mode, the rate) before measuring:
  latency measured on the wrong path is not the number M1 wants.
- `T-002e` displays the number; this task produces it and puts it in `SessionSnapshot`.
- An ADR is due if the estimation method closes off the alternatives (for example a loopback
  click measurement in-app).

## Acceptance criteria

- [ ] The cause of the output's fast-path refusal is found and fixed, or recorded as the device's
      behaviour with evidence (logcat, `dumpsys media.audio_flinger`)
- [ ] `SessionSnapshot` carries the round-trip estimate (and its parts: input, output), updated
      off the RT thread; host tests on the virtual clock cover the arithmetic
- [ ] On the Redmi, wired headphones and the built-in speaker/mic: the in-app estimate is within
      ±5 ms of OboeTester's round-trip median, or the gap is explained
- [ ] The numbers are recorded in the journal, with device, OS, route, rate and burst
- [ ] A stall's input stays queued as latency until a rebuild (`adr/0025`): the estimate shows it,
      and a test on the virtual clock covers it (owner since `T-015`)
- [ ] Any new RT call is in the audited list and its ADR; the allocation canary still fires
- [ ] `just verify` passes; `just test-android-device` debug and `--release` pass

## Out of scope

The calibration UI (M5). Bluetooth latency (record it if observed, do not optimise). iOS
(`T-002c`).

## Implementation notes

## Verification performed

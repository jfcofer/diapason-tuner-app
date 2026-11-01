---
id: T-026
title: Measure the detector and hear the microphone on the Redmi, before any wiring
status: todo
milestone: M2
owner: unassigned
created: 2026-11-01
---

## Goal

Nothing in `AUDIO_ENGINE.md` §1 has been measured on a phone, and every recording so far came from a
library, not a phone microphone. This is the cheapest test that can overturn the design: it needs no
threads and no UI. A failure here moves `adr/0028`'s hops, or the chord tests of `adr/0033` and
`adr/0034`, before anything is built on them.

## Context

Read: `docs/AUDIO_ENGINE.md` §1, `docs/adr/0030` (recordings), `docs/adr/0033`, `docs/adr/0034`,
`tools/test-android-device.sh`. Needs the Redmi attached and the owner playing a guitar. HyperOS wants
a tap on every USB install (`STATE.md`, Traps).

- **First step, unverified:** whether the criterion benches can run on the phone through
  `cargo-ndk-runner`. Reuse them: `dsp`'s lint list bans `Instant::now`, tests included.
- `chain/analysis` costs about 78 µs per 10 ms block on the host (`benches/baseline.json`), so 0.78 % of
  a core there. The phone's figure is what this task finds.
- The Redmi grants the VoiceRecognition preset and refuses Unprocessed (`T-002b`). `STATE.md`'s open
  questions already suspect its high-pass may hide a real low string under its harmonic.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] A `just` recipe runs the `dsp` benches on an attached device, or the task records why it cannot
      and what measures the same cost instead.
- [ ] Recorded on the Redmi, in release: frame cost per range (budget ≤ 1.5 ms, fail 4 ms) and the
      duty cycle of `Analysis` at its real hop (budget ≤ 8 % CPU, fail 15 %). The owed 4096-point frame
      from `T-003a` is included.
- [ ] A dev-only capture path records the microphone through `AAudioBackend` to a WAV. It is not in
      the prod flavour.
- [ ] The owner's captures: each open string, low E repeated, a fifth, a fourth, and strums. Replayed
      through `recorded.rs`, with per-frame results recorded. Any checked in follow `adr/0030`.
- [ ] The gate's outcome is written here, and STATE's order changed if it says so:
  - frame cost fails → revisit `adr/0028` by a new ADR before `T-027`;
  - a real single string withheld by `adr/0033`/`adr/0034` → fix that next: it is a live bug;
  - `T-023` and `T-003g` are re-scoped or abandoned from this evidence.

## Out of scope

Wiring the detector into the engine (`T-027`). Tuning thresholds in this task.

## Implementation notes

## Verification performed

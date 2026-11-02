---
id: T-027
title: Run the front end in the callback and carry its blocks across a ring
status: blocked
milestone: M3
owner: unassigned
created: 2026-11-01
---

**Blocked on `T-035` and `T-036`** (since 2026-11-02): `T-026` found a real low E misread and CPU on the A55 over
budget. `T-036`'s decision can change what crosses this ring.

## Goal

First link between `dsp` and the app: `chain::Front` runs in the audio callback, and what it produces
crosses a lock-free ring to whatever analyses it. `dsp::chain` has no caller outside its own tests
today. `engine::Processor` uses only `RmsMeter` and `SineOscillator`.

## Context

Read: `AGENTS.md` §6, `docs/ARCHITECTURE.md` §4, `docs/AUDIO_ENGINE.md` §2 and §7,
`rust/crates/engine/src/processor.rs`, `rust/crates/dsp/src/chain.rs`. The `dsp-engineer` agent's scope.

- **Block:** an `rtrb` ring of Copy `Block { frame: u64, rate: u32, gate: GateState, len: u16,
  samples: [f32; 256] }`. Fixed blocks avoid wrap-around at the consumer, and an 8192-frame callback
  splits into them.
- **Capacity:** 47 blocks, about 250 ms at 48 kHz. An estimate: check it against `T-026`'s frame cost.
- **Overflow:** drop the newest and count it, never block. The consumer sees the gap in `frame`.
- **`Front::new` allocates** (`NoiseGate`'s history), so it cannot run on a rate change in the
  callback. Build one per entry of `SUPPORTED_RATES` in `Engine::prepare` and swap in
  `follow_sample_rate`. Any other rate pushes nothing and reports the tuner unavailable. Ask AAudio for
  48 kHz so it resamples; that closes STATE's "rates other than 44.1/48" question for Android.
- **Across threads, `engine` may use only `rtrb` and `triple_buffer`** (`AUDIO_ENGINE.md` §7).

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] `engine/tests/no_alloc.rs` still passes with the front end running and the trap armed, and it
      covers a rate change and an overflow.
- [ ] Tests over `OfflineBackend`: irregular block sizes reassemble to the same samples; `frame` is
      continuous; an overflow is counted and shows as a gap in `frame`.
- [ ] A `Front` bench is added and reported against the callback budget in §1 (≤ 15 % of a buffer).
- [ ] A new ADR, `adr/0037`, records the ring, its overflow policy and the prebuilt fronts. `T-028`
      and `T-029` extend it before it merges, or supersede its open parts.
- [ ] The output-only path pushes nothing: the tone and the metronome never pay for the tuner.

## Out of scope

The consumer (`T-028`), any thread (`T-029`), any FFI or Dart change.

## Implementation notes

## Verification performed

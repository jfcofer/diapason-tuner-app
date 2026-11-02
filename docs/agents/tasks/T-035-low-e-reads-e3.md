---
id: T-035
title: A real low E on the phone shows E2, never E3 with its octave known
status: todo
milestone: M2
owner: unassigned
created: 2026-11-02
---

## Goal

On the Redmi, an open low E shows E3 with its octave marked known on 8–18 % of its frames, in five of six takes
(`T-026`). A beginner tuning that string to E3 would break it. `PRODUCT_SPEC.md` §2 promises the display does not
flip octaves, and no library recording ever showed this. Why they differ is part of this task.

## Context

Read: `T-026`'s capture table, `docs/AUDIO_ENGINE.md` §2–§3, `docs/adr/0029`, `docs/adr/0031`,
`rust/crates/dsp/src/range.rs` (`RangeSelector::observe_verifier`, `tests_octave`), `rust/crates/dsp/src/octave.rs`.
The `dsp-engineer` agent's scope. Rust DSP work, so it needs the owner's approval of a plan first (`AGENTS.md` §3).

- **What is known:** nearly every E3 frame is in High. In Mid the string reads E2. The takes' fundamental sits 12.8–
  17.0 dB under the second partial, where the library's E2s sit 0.1 above to 9.4 under.
- **What is not:** why the selector leaves Mid, why the Low and Mid verifiers do not bring it back, and why the
  guard calls E3's octave known. `e2_soft` (−12.8 dB) fails and `e2_hand` (−13.8 dB) barely does, on one take, so
  the ratio alone does not explain it. Nor is the phone shown to be the cause (`T-026`). `g3`'s six G4 frames, in
  High, may be the same bug: check them.
- **Fixtures need a decision first.** `adr/0030` rejected owner recordings, calls phone recordings a later task,
  has no set for them, and `xtask fixtures import` requires a library archive. A file may hold 1 MiB (about 7.3 s
  at 24-bit), with about 2.3 MiB free in all. The repo is public, and a phone hears the room.
- `MAX_HARMONIC`, `CHANGE_CENTS` and the leap rule were all set against library recordings and synthetic signals.
- Takes: `~/.local/share/diapason/captures/2026-11-02/` on the owner's machine. `just replay <wav> mid 40`.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] A new ADR, superseding that part of `adr/0030`, allows the owner's phone recordings: their licence, source
      and set, and what `xtask fixtures` does for them. The tooling matches it.
- [ ] A cut of a failing take, chosen by the owner, is in `fixtures/audio/` under that ADR, with its reference,
      and fails first. So does a cut of a take that passes today, as the control.
- [ ] Those fixtures gate: on low E, no frame shows E3 with its octave known, on the estimate or the needle.
- [ ] `g3`'s G4 frames are explained, and fixed if they are this bug.
- [ ] Every other take replays no worse, with the table in `T-026` re-measured and recorded here.
- [ ] Every library gate in `recorded.rs` and `tests/chain.rs` still holds, or the change is recorded.
- [ ] `just bench` and `just bench-android` show what the fix costs; the figure goes to `T-036`.

## Out of scope

Chords (`T-023`). The cost of `Analysis` (`T-036`), except what this adds.

## Implementation notes

## Verification performed

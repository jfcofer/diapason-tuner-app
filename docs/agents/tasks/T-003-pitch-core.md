---
id: T-003
title: Pitch core — detect f₀ offline to the accuracy budgets, before any tuner UI
status: in-progress
milestone: M2
owner: claude
created: 2026-10-22
---

## Goal

M2's exit is the accuracy the whole product rests on: every budget in `AUDIO_ENGINE.md` §1 met
against fixtures, no octave errors on pinch harmonics or a low B, committed bench baselines, a
reproducible fixture generator, and `dsp` agreeing with `core_domain` on the note table. This is a
parent task. Each slice below has its own file, PR and acceptance criteria. It runs host-only and
offline, so it can proceed while M1's device rows wait for hardware.

## Context

Read: `docs/AUDIO_ENGINE.md` §1–§4 and §8, `docs/adr/0005`, `docs/adr/0018`, `docs/adr/0021`,
`docs/TESTING.md` §2. The `dsp-engineer` agent's scope. `dsp` depends on no crate of ours, and on
third-party crates only from the list in `tools/check-deps.sh` (`AGENTS.md` §5).

## Slices

| Slice | Scope | Status |
|---|---|---|
| `T-003a` | NSDF/MPM core, synthetic fixture generator, ±0.5 ¢ on steady tones 30–1400 Hz, criterion bench and the regression gate, the no-alloc proof | in progress |
| `T-003b` | Pre-processing: 20 Hz DC block, adaptive RMS gate, ×4 FIR decimation with hysteresis, adaptive window per range. **Gates harmonic-rich tones (sawtooth) at ±0.5 ¢ through the chain:** its filter must cut well below the analysis Nyquist (`T-003a`'s notes) | todo, no file |
| `T-003c` | Octave guard (τ/2, 2τ, three-frame hysteresis), median-of-5 and one-euro, lock detection; pinch-harmonic and low-B tests | todo, no file |
| `T-003d` | `fixtures/note_table.json`; `dsp` and `core_domain` both assert against it (A4, temperaments, transposition) | todo, no file |
| `T-003e` | Recorded fixtures (guitar, bass, ukulele, violin), ±2 ¢. Needs someone to record them, under a licence `adr/0009` permits | blocked on the owner |

Wiring the detector into `engine` and the snapshot is M3, not this task.

## Acceptance criteria

- [ ] Every slice above is done, or abandoned with a reason
- [ ] The M2 exit criteria in `ROADMAP.md` hold, each with the test or bench that proves it

## Out of scope

UI of any kind. The engine and FFI wiring (M3). Metronome DSP (M4).

## Implementation notes

## Verification performed

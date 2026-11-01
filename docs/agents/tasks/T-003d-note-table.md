---
id: T-003d
title: Note table: 12-TET, A4 and transposition, shared by Rust and Dart
status: todo
milestone: M2
owner: unassigned
created: 2026-11-01
---

## Goal

`T-031` cannot name a note without this, and M2's exit needs both languages to agree. `core_domain` has
no note math today and `fixtures/note_table.json` does not exist. Temperaments are split out to `T-003h`.

## Context

Read: `docs/AUDIO_ENGINE.md` §4, `docs/adr/0007`, `docs/ARCHITECTURE.md` §3, `AGENTS.md` §5 (`core_domain` is
pure Dart; `dsp` takes third-party crates only from `tools/check-deps.sh`'s list). `serde_json` is already a
dev-dependency of `dsp`, so tests can read the table and the shipped crate does not link it.

- Only the lock needs a target in Rust; `dsp::lock` takes the caller's cents (`adr/0032`). Keep the Rust
  side as small as that need. Lossy casts stay in `dsp::convert` (`adr/0018`).
- Note names are a first-class concern (`PRODUCT_SPEC.md` §4), but the name systems are `T-033`'s UI. Here
  the table holds indices, and a Dart type turns one into English names only.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] `fixtures/note_table.json` holds, for A4 at 415, 440 and 466 Hz and transposition −12, 0 and +12, the
      expected note and cents at frequencies across the range the detector reads.
- [ ] `dsp` and `core_domain` each assert every row of the same file, including a note exactly between two
      semitones, and the range's ends.
- [ ] `core_domain` has public doc comments and no Flutter import.
- [ ] Rust DSP changes carry a fixture-based accuracy test (`AGENTS.md` §7).

## Out of scope

Temperaments, per-string offsets and name systems. Any wiring.

## Implementation notes

## Verification performed

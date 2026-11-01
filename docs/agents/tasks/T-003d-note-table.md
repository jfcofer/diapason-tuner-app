---
id: T-003d
title: Note table: 12-TET, A4 and transposition, shared by Rust and Dart
status: done
milestone: M2
owner: claude
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

- [x] `just verify` passes. CI's six jobs are this PR's own gate to merge.
- [x] `fixtures/note_table.json` holds, for A4 at 415, 440 and 466 Hz and transposition −12, 0 and +12, the
      expected note and cents at frequencies across the range the detector reads.
- [x] `dsp` and `core_domain` each assert every row of the same file, including a note exactly between two
      semitones, and the range's ends.
- [x] `core_domain` has public doc comments and no Flutter import.
- [x] Rust DSP changes carry a fixture-based accuracy test (`AGENTS.md` §7).

## Out of scope

Temperaments, per-string offsets and name systems. Any wiring.

## Implementation notes

**A tie needs a stated rule, and evidence showed why.** Rust's `log2` and Dart's `log(x) / ln2` differ in the
last bit. Across every 0.1 Hz reference from 415 to 466 Hz and notes 21 to 96, **3,002 of 38,836 exact ties
(7.7 %) rounded apart**: `log2` low in 34, `log / ln2` low in 2,968. So both add 1e-9 of a semitone, about a
ten-millionth of a cent, and a tie goes to the upper note. The first table passed without the tolerance, as
it held no such case. Six of those frequencies are now rows, which fail either language without it.

**Transposition:** shown = sounding + transposition. So a B♭ clarinet is +2 and a capo on fret 2 is −2, and
cents do not change. This was not decided anywhere, so `AUDIO_ENGINE.md` §4 now says it.

**Notes use MIDI numbers, and Dart holds the names.** `T-033` handles other name systems and flats. The Rust
side is only what `T-028` needs for `dsp::lock`'s cents. The field is `note` in Rust and `number` in Dart; the
table says `midi`.

**The table is one-off, computed in Python** apart from both implementations: a generator would be another
file to keep true, and its formula is in the file's description. `T-003h` extends it.

`dsp::convert::floor_to_i32` is the one new lossy cast (`adr/0018`).

**A gate changed, by the owner's decision (2026-11-01).** `adr/0007`, `TESTING.md` and `ARCHITECTURE.md` require
`core_domain`'s tests to read the table, and `check-deps` banned `dart:io` in its `test/` too. It now bans
`dart:io` only in `lib/`, and Flutter everywhere. It proves that on a planted package each run. Rejected:
moving the test to `core_platform`, away from the code, and generating a second copy of the data.

## Verification performed

2026-11-01. `just verify` passes. Both suites assert all 231 rows. Each mutation below fails a test, and each
was restored and checked with `cmp`:
- Rust: transposition's sign; rounding down, not to nearest; no tie tolerance.
- Dart: transposition's sign; no tie tolerance; the octave off by one.

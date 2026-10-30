---
id: T-003e
title: Recorded instrument fixtures, with an independent reference pitch, under §1's budgets
status: todo
milestone: M2
owner: claude
created: 2026-10-30
---

## Goal

Every accuracy test so far runs on synthetic signals (`adr/0027`). M2's exit needs recordings: §1
budgets recorded acoustic guitar at ±2 ¢ (fails at ±4), and the octave guard must hold on a real low
B. The owner chose published CC0 sample libraries over recording them (2026-10-30). This task
curates about 24 notes from them into `fixtures/audio/`, measures each one's pitch with a method
independent of MPM, and gates the detector against it.

## Context

Read: `docs/AUDIO_ENGINE.md` §1 and §8, `adr/0009`, `adr/0027`, `docs/TESTING.md` §2,
`rust/crates/dsp/tests/accuracy.rs` (`an_inharmonic_pluck_is_recorded`: the truth for a string is
its first partial), `rust/crates/xtask/src/main.rs`. Dsp work goes to the `dsp-engineer` agent.

**Sources** (audited 2026-10-30; archives in `~/.cache/diapason/sound-sources/`, never in the repo):

| Library | Licence | Use | Format |
|---|---|---|---|
| Meta Steel / Nylon Guitar v1.0.0 (WAV release assets, not the repo's OGG previews) | CC0 | gold | 48 kHz 24-bit stereo |
| Karoryfer Emilyguitar v1.001, `notes/` only | CC0 | ceiling | 44.1 kHz 24-bit |
| Karoryfer Black and Blue Basses 1.002, `darkblack/reg/` | CC0 | ceiling; B0 octave gate | 44.1 kHz 24-bit mono |
| FreePats Ukulele 2026-08-11 | CC0 | ceiling | 48 kHz 24-bit mono |
| VSCO 2 CE, `Strings/Solo Violin/` | CC0 | `Pizz` ceiling, `Arco Vib` stress | 44.1 kHz 16-bit stereo |

**Traps, measured:**
- Karoryfer bass names are an octave high: `darkblack_b1_*` measures 30.87 Hz (B0) and `e2` E1.
  `babyblue/fake_*` is pitch-shifted or a detuned string: exclude it.
- Note names are not pitch: Emilyguitar `e2_f_rr1` is +13.5 ¢ from E2.
- Rejected sources: FreePats steel guitar (GPL), IDMT-SMT-Bass (NC-ND), NSynth (16 kHz).

## Acceptance criteria

- [ ] `adr/0030` decides licences for checked-in test assets (CC0, CC-BY-4.0 with credit, or an
  explicit unrestricted grant), the format (PCM WAV, one source channel, native rate and depth, no
  resampling, normalising or dithering), storage (plain git, ≤ 1 MiB a file, ≤ 10 MiB in all) and
  truth (the first partial, independently measured, ≤ 0.5 ¢ uncertainty for gold).
- [ ] `fixtures/audio/` holds the WAVs, `manifest.json` (source, member, SHA256s, licence evidence,
  channel, trim, rate, depth, reference with method and uncertainty) and `LICENSES/`, one verbatim
  licence file per library.
- [ ] `cargo xtask fixtures import` cuts a fixture from an archive member; `fixtures reference`
  measures it by a method that shares no code with MPM; `fixtures check` validates hashes,
  headers, licence evidence, size caps and coverage, and runs in `just verify`.
- [ ] `fixtures check` fails on a flipped byte and on a missing licence file.
- [ ] `dsp/tests/recorded.rs` runs each fixture through `chain::Front` and `chain::Analysis` at its
  native rate. Gold acoustic guitar: the median over the reference window within §1's ±2 ¢, zero
  octave errors. Bass B0: zero octave errors. The rest: under regression ceilings owned by
  `T-003g`.
- [ ] Three references (steel E2, bass B0, ukulele A4) cross-checked against an external tool.
- [ ] `AUDIO_ENGINE.md` §8, `TESTING.md` §2 and `REPO_LAYOUT.md` describe what was built.
- [ ] `just verify` passes, and all six CI jobs are green.

## Out of scope

Phone-microphone recordings (with the Redmi, in device work). Budgets for instruments §1 does not
name (`T-003g`). Smoothing and display stability (`T-003f`).

## Implementation notes

## Verification performed

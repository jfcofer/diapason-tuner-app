---
id: T-003e
title: Recorded instrument fixtures, with an independent reference pitch, under §1's budgets
status: done
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
`rust/crates/dsp/tests/accuracy.rs` (`an_inharmonic_pluck_is_recorded` records a synthetic pluck
against its first partial; recordings are held to the harmonic consensus instead, `adr/0030`),
`rust/crates/xtask/src/main.rs`. Dsp work goes to the `dsp-engineer` agent.

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

- [x] `adr/0030` decides licences for checked-in test assets (CC0, CC-BY-4.0 with credit, or an
  explicit unrestricted grant), the format (PCM WAV, one source channel, native rate and depth, no
  resampling, normalising or dithering), storage (plain git, ≤ 1 MiB a file, ≤ 10 MiB in all) and
  truth (the harmonic consensus, independently measured, ≤ 0.5 ¢ uncertainty for gold).
- [x] `fixtures/audio/` holds the WAVs, `manifest.json` (source, member, SHA256s, licence evidence,
  channel, trim, rate, depth, reference with method and uncertainty) and `LICENSES/`, one verbatim
  licence file per library.
- [x] `cargo xtask fixtures import` cuts a fixture from an archive member; `fixtures reference`
  measures it by a method that shares no code with MPM; `fixtures check` validates hashes,
  headers, licence evidence, size caps and coverage, and runs in `just verify`.
- [x] `fixtures check` fails on a flipped byte and on a missing licence file.
- [x] `dsp/tests/recorded.rs` runs each fixture through `chain::Front` and `chain::Analysis` at its
  native rate. Gold acoustic guitar: the median over the reference window within §1's ±2 ¢, zero
  octave errors. Bass B0: zero octave errors. The rest: under regression ceilings owned by
  `T-003g`.
- [x] Three references (steel E2, bass B0, ukulele A4) cross-checked against an external tool
  (numpy's FFT; the script is not in the repo, see notes).
- [x] `AUDIO_ENGINE.md` §8, `TESTING.md` §2 and `REPO_LAYOUT.md` describe what was built.
- [x] `just verify` passes, and all six CI jobs are green (the ruleset merges nothing otherwise).

## Out of scope

Phone-microphone recordings (with the Redmi, in device work). Budgets for instruments §1 does not
name (`T-003g`). Smoothing and display stability (`T-003f`).

## Implementation notes

- **Selection** (24 notes, 7.6 MB): steel E2 A2 G3 B3 E4 and nylon E2 D3 B3 E4 (gold); Emilyguitar
  E2 A2 A4; bass B0 E1 A1 D2; ukulele G4 C4 E4 A4; violin pizzicato G3 A4 E5 (ceiling); violin arco
  vibrato A4 (stress). Meta's WAV release assets were fetched with `gh`. They matched GitHub's
  digests, and every member matched their own `SHA256SUMS`. Channel: left on Meta (L/R correlation
  0.95–1.00), right on VSCO (the louder of two microphones).
- **Deviation, the truth** (owner, 2026-10-30): the plan held recordings to the first partial.
  Against it, five of nine gold notes missed ±2 ¢ (steel E2 +5.4), each on the side the upper
  partials sit, because the guitar body pulls the first partial off the series. `adr/0030` was
  revised on its branch: the truth is now the amplitude-weighted consensus of `f_k / k` over
  partials 1–6. `first_partial_hz` stays in the manifest.
- **Results against the consensus:** gold within ±2 ¢ (worst steel E2 +1.82, steel B3 +1.52); bass
  B0 −0.04 ¢ with zero gross frames; every ceiling note's median within 2.6 ¢.
- **Found:** `ukulele_g4` shows 20 gross frames, a third of G4 marked `Known` once the selector
  drops to Mid. That is now `T-020` (owner, 2026-10-30), and the note's ceiling comment names it.
- **The kit's Python** was ported to `cargo xtask fixtures`, not kept; its paths moved to
  `fixtures/audio/`. Its claim that `adr/0009` held an asset whitelist was wrong; `adr/0030` is
  that whitelist now.
- `steel_g3` uses window 700–1700 ms: at 300–1300 it glides 11 ¢ and its uncertainty was 0.506.
- `.gitattributes` marked all of `fixtures/audio/**` binary, which hid the manifest's diffs. Now
  only `*.wav` is binary.
- `cargo xtask` had no alias: `.cargo/config.toml` adds it.
- **Review fixes** (`reviewer`, 2026-10-30):
  - Gold and ceiling notes now fail on any unvoiced frame. Before, an over-withholding guard
    passed.
  - `.gitattributes` keeps `LICENSES/` byte for byte (two files are CRLF), and `evidence_sha256`
    pins each file.
  - `check` requires gold if and only if acoustic guitar, pinned archive and member hashes, and
    `file` = `<id>.wav` and `evidence` = `LICENSES/<file>`, never outside the directory.
  - `check` re-derives every reference from its WAV (0.01 ¢ tolerance). A hand edit fails, and
    the `demodulate` phasor and xtask's dev `opt-level` keep it to about 2 s.
  - Silence is refused rather than turned into NaN.
- **The cross-check is not re-runnable from the repo:** a numpy script is a second toolchain.
  `check`'s re-derivation guards the stored values; independence rests on the recorded result.

## Verification performed

2026-10-30, host. `cargo xtask fixtures check` passes, and its tests fail on a flipped byte, a
missing licence file, a GPL licence, an unlisted WAV and an imprecise gold reference.
`recorded.rs`: four tests pass, about 2 s. A separate numpy FFT (one long Hann window, Gaussian
peak interpolation per partial) matched the consensus references to within 0.08 ¢ on steel E2,
bass B0, ukulele A4, nylon E4 and steel A2. It matched the first-partial references to within
0.11 ¢ before the revision.

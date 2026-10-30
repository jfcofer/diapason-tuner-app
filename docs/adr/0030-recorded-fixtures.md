# 0030 — Recorded fixtures: CC0 libraries, cut exactly, held to an independent harmonic consensus

**Status:** Accepted · 2026-10-30 (sources chosen by the owner over recording them, and the
harmonic consensus chosen as the truth, both 2026-10-30)

## Context

`adr/0027` generates synthetic fixtures in code and checks in only recordings, leaving their format
and how they are read to `T-003e`. `adr/0009` governs dependencies and fonts, not test assets. The
owner chose published sample libraries over recording the instruments (2026-10-30), and downloaded
3.4 GB of them. Three findings from auditing them shape what follows:

- Library note names are not pitch. Karoryfer's bass files are named an octave high
  (`darkblack_b1` measures 30.87 Hz, B0). The notes themselves sit up to ±11 cents from nominal.
- On recorded guitars the first partial is not where the harmonic series points. On steel E2,
  partials 2–6 sit 1.6 to 7.7 cents sharp of it, the second 8 dB louder than the first. On nylon
  E4 they sit 6.5 to 9 cents flat. Held to the first partial, the detector missed ±2 cents on five
  of nine notes (steel E2 by 5.4), each time on the side the upper partials sit. The truth has to
  be chosen.
- A repository snapshot can hold only lossy previews (Meta Open Sound: 1583 OGGs, no WAV). Only a
  release asset is a source.

## Decision

- **Licences.** A checked-in test asset is CC0-1.0, CC-BY-4.0 with a credit line in the manifest,
  or under a publisher's explicit grant of use without conditions (`LicenseRef-unrestricted`). GPL,
  NC and ND are rejected, and so is any source whose licence file is not in the archive.
  Fixtures are never shipped, so `adr/0009`'s in-app licence page does not list them.
- **Evidence.** Each library's licence file is copied verbatim into `fixtures/audio/LICENSES/`.
  The manifest pins the archive's SHA-256, the member's path and SHA-256, and the output's SHA-256.
  Archives stay outside the repository.
- **Format.** RIFF PCM WAV, one channel of the source, never a mix. Native rate (44.1 or 48 kHz)
  and bit depth. Cut on frame boundaries, never resampled, normalised, dithered or faded.
- **Storage.** Plain git, no LFS: at most 1 MiB a file and 10 MiB in all.
- **Truth: the harmonic consensus** (owner, 2026-10-30): the amplitude-weighted mean of `f_k / k`
  over the first six partials above a −30 dB floor. For a low note the ear's pitch comes from the
  second to fifth partials, and chords beat where partials coincide, so this is the pitch a
  player tunes to. Each partial is measured by a method that shares no code with the detector:
  phase advance between Hann frames at a demodulation frequency. The statistic is the median over
  a declared window. Its uncertainty is the median's standard error after removing the window's
  linear glide. The first partial is recorded beside it. The measurement is refused if the label
  is an octave off either way, or the window is silent. This is the "measured f₀" `adr/0027` asks
  every recording to carry.
- **Sets.** *Gold* fixtures (acoustic guitar, the only recording `AUDIO_ENGINE.md` §1 budgets) need
  an uncertainty ≤ 0.5 cents and are held to §1. *Ceiling* fixtures (electric guitar, bass,
  ukulele, violin pizzicato) are held under regression ceilings until `T-003g` sets their budgets.
  The bass low B is held to zero gross frames now, since that is M2's exit criterion. *Stress*
  fixtures (vibrato) are reported only.
- **Tooling.** `cargo xtask fixtures import | reference | check`, in Rust, so the repo has no second
  toolchain. `check` runs in `just verify`. `hound`, `serde_json` and `sha2` are tool and test
  dependencies only.

## Consequences

- A cut and its source can be re-derived from the manifest by anyone with the archive. `check`
  re-derives every truth from the committed WAV, so a hand-edited pitch fails `verify`.
- Accuracy now has a ground truth that is not the detector's own idea of pitch. Against it, all
  nine gold notes are within ±2 cents (worst +1.8), and every ceiling note's median within 2.6.
  Ukulele G4 still shows 20 gross frames (`T-020`). The stress note reads +5.0.
- `dsp/tests/accuracy.rs` still records its synthetic pluck against the first partial. A pluck
  model has no body to pull the first partial off the series, so there the two differ only by
  inharmonicity.
- Each new instrument family needs a library, a licence file and a set. Phone-microphone recordings,
  with room and device colour, are a later task.

## Alternatives considered

- **The owner records the instruments:** fewer sources, but a home recording's pitch is no better
  known, and the owner preferred professional libraries.
- **The library's note names as truth:** wrong by an octave on one library, and by up to 11 cents
  everywhere.
- **The first partial as truth:** what a strobe's lowest band shows. It measures the guitar's
  body as much as the string. It sits up to 3.6 cents from the consensus on these guitars, and
  4.5 on the bass.
- **Periodicity (YIN) as truth:** it is the quantity under test, measured the way the detector
  measures it.
- **Fitting `f_k = k·f0·√(1+Bk²)` for `f0`:** this extrapolates to a fundamental that no partial
  sits at, below them all.
- **Git LFS:** quotas and an extra tool for every clone, for 8 MB.
- **The kit's Python tools:** a second toolchain for `just doctor` to pin; `xtask` exists for this.

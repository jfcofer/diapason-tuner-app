# 0027 — Synthetic audio fixtures are generated in code; only recordings are checked in

**Status:** Accepted · 2026-10-22

## Context

`AUDIO_ENGINE.md` §8 and `REPO_LAYOUT.md` planned for every audio fixture to be written by
`cargo xtask fixtures` and checked in as a small FLAC under `fixtures/audio/`, "deterministic,
small, and CI must not synthesise them". That covered two kinds of signal:

- **synthetic** ones: sines, sawtooths and a stiff-string pluck, whose f₀ is a parameter;
- **recorded** ones: real instruments at known pitches, whose f₀ was measured once.

`T-003a` is the first task to need fixtures, and all of them are synthetic. Checking those in as
FLACs has a cost:

- `dsp` would need a FLAC decoder as a dev-dependency, and `xtask` an encoder.
- Every change to the generator would mean regenerated binaries in a diff that nobody can review.
- The accuracy sweep wants hundreds of frequencies (24 or more per octave from 30 Hz to 1400 Hz),
  times several signal types. That is far more audio than "small".

The worry behind "CI must not synthesise them" is a generator that drifts along with the detector,
so that both agree and both are wrong. A synthetic fixture's expected f₀ is the generator's input,
though, not a value stored next to its output. The generator can then be tested on its own, the way
`osc` already is, by measuring its output independently of the detector.

## Decision

- **Synthetic fixtures are generated in code, at test time,** by `diapason_dsp::synth`. They are
  deterministic: noise comes from a seeded generator, and no wall clock or OS entropy is involved.
- `synth` is tested on its own terms. Its frequencies are checked by a measurement that does not
  use the pitch detector.
- The expected f₀ in every accuracy test is the parameter the signal was generated from.
- **Only recordings are checked in**, under `fixtures/audio/`, each with its measured f₀ and how it
  was measured. They arrive in `T-003e`, which picks their format and how they are read.
- `cargo xtask fixtures` no longer synthesises anything. `T-003e` decides whether it imports
  recordings or is removed.

## Consequences

**Good.**
- A sweep can be as dense as the budget needs at no cost to the repository's size.
- A change to the generator is a reviewable code diff.
- No codec dependency until there is a recording to decode.

**Bad.**
- Test time includes synthesis, which is cheap next to the analysis it feeds.
- Floating-point maths (`sin`, `exp`) can differ in the last bit between platforms, so fixtures
  are not bit-identical across hosts. The accuracy tests assert a tolerance in cents, never exact
  samples, so this does not matter for them. Any future test that wants bit-exactness must check in
  its input instead.

**Rejected: FLACs for everything, as planned.** The cost is above. Nothing a synthetic FLAC would
prove is missing from a tested generator.

**Rejected: WAVs written to a cache by the test run.** This gives the same determinism with more
moving parts, plus a stale-cache failure mode.

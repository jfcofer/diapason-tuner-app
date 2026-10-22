---
id: T-003a
title: MPM pitch detector, synthetic fixtures, and the bench gate
status: in-progress
milestone: M2
owner: claude
created: 2026-10-22
---

## Goal

`dsp` gets its first real detector: the McLeod Pitch Method (`adr/0005`), allocation-free and
panic-free per frame, accurate to ±0.5 cent on steady synthetic tones from 30 to 1400 Hz. It lands
with the fixtures that prove it and with the criterion bench whose 10 % regression gate has been
wired since `T-001` but has never had anything to measure. Everything later in `T-003` builds on
this core, so it must be measured, not assumed.

## Context

Read: `docs/AUDIO_ENGINE.md` §1, §3 and §8, `docs/adr/0005`, `docs/adr/0018` (lossy casts only in
`dsp::convert`), `docs/adr/0021` (panic-free), `docs/TESTING.md` §2.

- Prior art: `dsp::level` and `dsp::osc` show the crate's style. `engine/tests/no_alloc.rs` is the
  pattern for an allocation trap with a canary in a child process.
- `dsp/clippy.toml` links to the engine's RT list: `Vec::push`, `format!` and the rest are already
  denied crate-wide.
- `tools/docs-check.sh` rejects every `#[allow]`/`#[expect]` outside `dsp/src/convert.rs`.
- `tools/bench-compare.sh` is a stub that fails once a bench exists.
- Dependency licences must be on `deny.toml`'s list. Check each API against docs.rs for the pinned
  version (`AGENTS.md` §3.3).

## Acceptance criteria

- [ ] `diapason_dsp::pitch::Mpm` implements `AUDIO_ENGINE.md` §3, steps 1–6:
  - FFT autocorrelation of the zero-padded window;
  - `m(τ)` incrementally, in O(N);
  - the NSDF;
  - key maxima, and the first one above `k · max` (`k = 0.9`, configurable);
  - parabolic interpolation;
  - clarity. Below 0.6, the result is `None`.
- [ ] Every buffer and FFT plan is allocated in the constructor. Per-frame analysis does not allocate
  (proven by `assert_no_alloc`, with a canary that shows the trap fires) and cannot panic:
  `unwrap_used`, `expect_used`, `panic` and `indexing_slicing` are denied outside tests, as in
  `session`.
- [ ] `diapason_dsp::synth` is a deterministic, seeded fixture generator. It provides:
  - a sine;
  - a band-limited sawtooth;
  - a stiff-string pluck, with partials at `n·f₀·√(1+B·n²)` that decay;
  - seeded white noise.
- [ ] An accuracy sweep over 30–1400 Hz, at ≥ 24 points per octave and 48 kHz, asserts ±0.5 ¢ on the
  sine and on the sawtooth. Each case uses the smallest power-of-two window that holds three
  periods. A failure names the frequency, the error in cents and the clarity.
- [ ] Clarity: noise alone gives `None`. A sine at 20 dB SNR gives clarity > 0.9.
- [ ] The inharmonic pluck's errors are printed and recorded here, not gated. Their budget belongs to
  `T-003c` and `T-003e`.
- [ ] A property test: across the range, and for gains from −40 to 0 dBFS, the estimate stays within
  budget and does not depend on the absolute gain.
- [ ] A criterion bench, `dsp/benches/nsdf.rs`, covers windows of 2048, 4096 and 8192.
  `tools/bench-compare.sh --save` records the baseline, and a default run fails on a regression
  above 10 %. The baseline names the host it was measured on.
- [ ] The host per-frame cost is recorded below. The phone budget (≤ 1.5 ms for 4096) is a device
  measurement still owed, not a claim.
- [ ] An ADR records that synthetic fixtures are generated in code and only recordings are checked
  in, and `AUDIO_ENGINE.md` §8 links it
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

Pre-processing, decimation and adaptive windows (`T-003b`). Octave guard, smoothing and lock
(`T-003c`). The note table (`T-003d`). Recorded fixtures (`T-003e`). Any engine, FFI or UI change.

## Implementation notes

## Verification performed

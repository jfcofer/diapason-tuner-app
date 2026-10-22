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

- [x] `diapason_dsp::pitch::Mpm` implements `AUDIO_ENGINE.md` §3, steps 1–6:
  - FFT autocorrelation of the zero-padded window;
  - `m(τ)` incrementally, in O(N);
  - the NSDF;
  - key maxima, and the first one above `k · max` (`k = 0.9`, configurable);
  - parabolic interpolation;
  - clarity. Below 0.6, the result is `None`.
- [x] Every buffer and FFT plan is allocated in the constructor. Per-frame analysis does not allocate
  (proven by `assert_no_alloc`, with a canary that shows the trap fires) and cannot panic:
  `unwrap_used`, `expect_used`, `panic` and `indexing_slicing` are denied outside tests, as in
  `session`.
- [x] `diapason_dsp::synth` is a deterministic, seeded fixture generator. It provides:
  - a sine;
  - a band-limited sawtooth;
  - a stiff-string pluck, with partials at `n·f₀·√(1+B·n²)` that decay;
  - seeded white noise.
- [x] (Deviations 1 and 2: §2 windows; sawtooth to `T-003b`.) An accuracy sweep over 30–1400 Hz, at
  ≥ 24 points per octave and 48 kHz, asserts ±0.5 ¢ on the
  sine and on the sawtooth. Each case uses the smallest power-of-two window that holds three
  periods. A failure names the frequency, the error in cents and the clarity.
- [x] Clarity: noise alone gives `None`. A sine at 20 dB SNR gives clarity > 0.9.
- [x] The inharmonic pluck's errors are printed and recorded here, not gated. Their budget belongs to
  `T-003c` and `T-003e`.
- [x] A property test: across the range, and for gains from −40 to 0 dBFS, the estimate stays within
  budget and does not depend on the absolute gain.
- [x] A criterion bench, `dsp/benches/nsdf.rs`, covers windows of 2048, 4096 and 8192.
  `tools/bench-compare.sh --save` records the baseline, and a default run fails on a regression
  above 10 %. The baseline names the host it was measured on.
- [x] The host per-frame cost is recorded below. The phone budget (≤ 1.5 ms for 4096) is a device
  measurement still owed, not a claim.
- [x] An ADR records that synthetic fixtures are generated in code and only recordings are checked
  in, and `AUDIO_ENGINE.md` §8 links it
- [x] `just verify` passes; all six CI jobs green (#30)

## Out of scope

Pre-processing, decimation and adaptive windows (`T-003b`). Octave guard, smoothing and lock
(`T-003c`). The note table (`T-003d`). Recorded fixtures (`T-003e`). Any engine, FFI or UI change.

## Implementation notes

Built by the `dsp-engineer` agent, then reviewed (2026-10-22).

- `Mpm::analyze(&[f32], rate) -> Option<PitchEstimate>`: FFT length the power of two ≥ 2N (linear
  correlation), lags to N/2. **realfft 3.5** APIs checked in source; **criterion 0.8**, no defaults.
- **f64 in the FFTs and the NSDF.** f32 is 25–29 % faster, but its error at 30–60 Hz is 20 times
  larger (0.0005 → 0.011 ¢), and its gain dependence (3·10⁻³ ¢) fails the property test.
- **`dsp` now has a dependency.** "dsp depends on nothing" meant "no crate of ours". `check-deps`
  now holds an explicit list (`realfft`), and `AGENTS.md` §5 says so.
- **The bench gate compares medians, and it needs a quiet host.** Under load from parallel agents,
  identical code moved by up to +20 %. Alone on the host, two runs agreed within 0.3 %. The gate
  runs locally, not in `verify` or CI (`CI_RELEASE.md` §1).
- `jq` is doctor-checked now. `synth::sine` does not reuse `osc`, which ramps in over 5 ms.
- The RT clippy list denies `println!`/`format!` in tests too: the summary prints only under
  `--nocapture`, and the property test uses `assert!`, not `prop_assert!`.

**Deviation 1: the gated sweep uses §2's windows, not three periods of each tone.** The criterion
below said "the smallest window that holds three periods". `AUDIO_ENGINE.md` §2 sizes a window to
three periods of the lowest note *of the range*: 8192 below 60 Hz, 4096 from 60 to 160 Hz, and 1024
from 160 Hz up, at 48 kHz.

A 640k-case probe found the per-tone sine at −0.507 ¢ (1299 Hz, window 128), between the old
grid's points: the random-seeded property test failed ~1 run in 160. Under §2 the worst is −0.065 ¢.
The per-tone stress sweep stays as recorded tables under ceilings:

| Table | Ceiling | Dense worst |
|---|---|---|
| sine, per tone | 1 ¢ | −0.507 ¢ |
| sawtooth, §2 | 4 ¢ | −2.86 ¢ |
| sawtooth, per tone | 8 ¢ | −6.86 ¢ |

**Deviation 2: a full-band sawtooth misses ±0.5 ¢, and this slice does not gate it.** There are two
causes, both reproduced in an independent prototype:

1. Its NSDF peak, `Σ wₕ cos(hω(τ−T))` with `wₕ ∝ 1/h²`, takes curvature equally from every harmonic
   up to Nyquist. So it is not a parabola over ±1 lag, and the vertex lands up to 0.053 lag off,
   whatever the window.
2. With short windows, harmonics near Nyquist distort the NSDF samples themselves.

Five remedies were measured (spectral interpolation, bias correction, a lag-0 template, windowed
sinc, a pre-NSDF low-pass); each broke the sine or fell short.

The bias scales with how close the content reaches the **analysis** Nyquist: 0.05 lag at full band,
0.013 below fs/4, and 0.0035 below fs/8. So the budget moves to where the cure lives: `T-003b`
gates harmonic-rich tones at ±0.5 ¢ through a decimation filter that cuts well below the new
Nyquist. `AUDIO_ENGINE.md` §1 is unchanged.

**Review fixes (2026-10-22).** The `reviewer` agent found these problems, and the engineer fixed each
one. Every fix was proven by a mutation that turns its test red:
- the threshold `k` was untested;
- the key-maximum scan spun forever on a NaN;
- the floor guard in `normalise` was dead code (m(τ) ≥ Σx² for τ ≤ N/2);
- the plateau rule was untested;
- `check-deps` could be bypassed (it now reads `cargo metadata` and fails closed);
- `harmonics_below_nyquist` could loop without bound;
- the pluck summed in f32;
- false doc claims ("bit-identical", "64-bit on every target");
- `bench-compare`: a kernel-release host key, corrupt input exiting 1, `--save` adopting stale runs.

## Verification performed

2026-10-22, Fedora 44 host (Ryzen 7 8840HS), rustc 1.98.0. Host only; no device was attached.

- `just verify` passed; nextest ran 91 tests, all passing, plus 15 doctests.
- Accuracy under §2 windows, from a dense probe of 640,032 cases: sine worst −0.065 ¢. The other
  numbers are in the deviations above.
- Clarity: noise alone peaks at 0.336, below the 0.6 floor. A sine at 20 dB SNR reads ≥ 0.9897.
- The pluck (B = 10⁻⁴) reads +2.2 to +3.0 ¢ against `f₁ = f₀√(1+B)`, at clarity ≈ 0.999. This is
  recorded, not gated (`T-003c`, `T-003e`).
- Host frame cost, as medians: 13.2 µs (2048), 28.6 µs (4096) and 58.9 µs (8192). Two quiet runs
  agreed within 0.3 %. **The phone figure (≤ 1.5 ms for 4096) is still owed.**
- Each gate was proven by breaking it:
  - a detuned expectation fails the sweep;
  - a `vec!` in `analyze` aborts the no_alloc test;
  - a planted slowdown fails `just bench`;
  - the reviewer's mutations (on `k`, the plateau, the NaN, and L = N) each fail a test;
  - `check-deps` catches `num-complex`, `[dependencies.rand]` and a target-specific `libc`.

# 0005 — MPM/NSDF for pitch detection

**Status:** Accepted · 2026-08-25

## Context

Candidates: FFT peak-picking, plain autocorrelation, YIN and its variants (pYIN), cepstrum, HPS, or
a learned model. The signal is a plucked or bowed string: strong harmonics, inharmonicity from
string stiffness, an attack transient, and decay. The hard requirements are ±0.5 cent on steady
tones, no octave errors, low CPU, and a usable confidence measure for the UI.

## Decision

The McLeod Pitch Method: normalised square difference function computed via FFT autocorrelation,
key-maximum picking with a 0.9 threshold, parabolic interpolation for sub-sample precision, plus an
explicit octave guard and hysteresis.

## Consequences

**Good.** The NSDF peak value is a natural clarity/confidence measure in [0, 1], which the UI needs
anyway for the lock indicator and the silence gate — with FFT or cepstral methods that signal has to
be invented separately. Parabolic interpolation reaches sub-cent resolution without huge windows.
FFT acceleration keeps it O(N log N). Fully deterministic and testable against fixtures.

**Bad.** Still susceptible to octave errors on pinch harmonics and very quiet low notes without the
explicit guard, which is extra code and extra tests. Long windows are needed for low bass, which
costs latency there — mitigated by adaptive window sizing.

**Rejected: FFT peak-picking.** Frequency resolution at a usable window length is nowhere near one
cent, and the loudest partial is often not the fundamental on a guitar.

**Rejected: pYIN.** More robust in a research sense, materially more expensive, and its probabilistic
output is the wrong shape for a real-time single-source display.

**Rejected: a learned model.** Binary size, latency variance, non-determinism in tests, and no
plausible accuracy gain on monophonic string tone. Reconsider only for polyphonic chord tuning.

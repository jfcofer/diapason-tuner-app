# 0008 — Synthesise metronome clicks; ship no audio assets in 1.0

**Status:** Accepted · 2026-08-25

## Context

A metronome needs distinguishable sounds for accent, beat and subdivision. The usual approach ships
recorded samples: woodblock, cowbell, digital beep.

## Decision

Generate clicks procedurally in `dsp`: an exponentially decaying sine/triangle blend plus an optional
filtered-noise transient, with different frequencies and envelopes per role. No audio files in the
bundle for 1.0.

## Consequences

**Good.** No sample licensing questions at all. Binary stays small. Timbre becomes continuous
parameters — a "softness" slider is a few lines rather than a new sample set. Fully deterministic, so
a scheduling test can assert on exact sample values. No decoder, no file I/O anywhere near the RT
thread, no asset loading during playback.

**Bad.** A synthesised click does not sound like a real woodblock, and some users will want one.
Getting a *pleasant* synthetic click takes real DSP taste and iteration; a bad one is fatiguing over
an hour of practice, which is the actual use case.

**Mitigation.** `ClickSource` is a trait from day one, so sample-based voice packs are an added
implementation post-1.0 rather than a rewrite. Budget explicit iteration time on click timbre in M4
— it is a design problem wearing an engineering costume.

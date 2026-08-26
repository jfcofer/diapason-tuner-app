# 0001 — Rust owns the entire audio path

**Status:** Accepted · 2026-08-25

## Context

A tuner needs microphone → pitch estimate with bounded latency. A metronome needs beats placed on
exact sample indices, with zero drift over an hour. Flutter offers three plausible splits: Dart
captures audio and calls Rust for analysis; a platform plugin captures and schedules while Rust does
maths; or Rust owns the stream end to end and Dart only renders.

## Decision

Rust owns everything from the device callback to the produced samples. Dart sends commands and reads
a ~30 Hz snapshot. **No audio buffer ever crosses the FFI boundary into Dart.**

## Consequences

**Good.** The latency path contains no garbage collector, no event loop, and no platform-channel
hop. Metronome scheduling can be sample-exact because the scheduler runs inside the callback that
fills the buffer. The hardest code in the project is testable offline, faster than real time, on any
machine, with no device and no Flutter. Desktop and CLI harnesses come free.

**Bad.** Rust changes require a native rebuild, so hot reload does not cover them — the inner loop
for engine work is `cargo test`, not the app. Two toolchains in CI. Platform audio bugs are debugged
in Rust with worse tooling than Kotlin or Swift would give. Contributors need both languages.

**Rejected: Dart captures, Rust analyses.** Puts the GC and channel hop inside the latency path,
makes sample-accurate scheduling impossible, and would have forced a second timing model for the
metronome.

**Rejected: platform plugins own audio.** Two native implementations to keep in sync, the DSP still
needs a home, and the interesting logic ends up split across three languages.

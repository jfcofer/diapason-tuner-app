# Diapasón

A production-grade **instrument tuner and metronome** for Android and iOS — phone and tablet.

Flutter for the interface, Rust for every sample of audio. Offline, no accounts, no telemetry,
no network calls.

> **Diapasón**: in Spanish, both *tuning fork* and *fretboard*. The name is a placeholder you can
> change — see "Renaming" below — but it earns its keep.

---

## Status

**Pre-implementation.** This repository currently contains the architecture, specifications, and
agent-operating system for the project. No application code has been written yet. The first task
(`docs/agents/tasks/T-001-scaffold.md`) creates the workspace skeleton.

Start here → **[`docs/agents/STATE.md`](docs/agents/STATE.md)**

## What is planned

**Tuner** — chromatic and instrument-specific (guitar, bass, ukulele, violin family, mandolin,
banjo), ±1 cent accuracy on steady tones, sub-60 ms perceived latency, a strobe display that
physically stalls at pitch, configurable A4 (415–466 Hz), historical and sweetened temperaments,
and note naming in letters, fixed-do solfège, or German.

**Metronome** — drift-free sample-accurate scheduling, arbitrary time signatures, subdivisions,
per-beat accents, tap tempo, tempo trainer, count-in, gap trainer, background playback, haptics
locked to the audio clock rather than to a UI timer.

Full requirements: [`docs/PRODUCT_SPEC.md`](docs/PRODUCT_SPEC.md).

## Architecture in one paragraph

A pure-Rust DSP core (`rust/crates/dsp`) with no I/O and no allocation in its hot path, driven by a
real-time engine (`rust/crates/engine`) that owns the audio callback, fed by swappable platform
backends (`rust/crates/audio_io`: Oboe/AAudio on Android, AudioUnit on iOS, cpal on desktop, and an
offline backend for deterministic tests). A thin flutter_rust_bridge layer (`rust/crates/ffi`)
exposes commands in and a ~30 Hz state snapshot out — audio buffers never cross into Dart. Flutter
consumes that stream through a feature-first package graph with a one-way dependency rule. Full
reasoning and diagrams: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Quick start

```bash
just setup     # toolchains, dependencies, codegen
just verify    # the gate: format, lint, test, drift check — Dart and Rust
just run ios   # or: just run android
```

Prerequisites and versions: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Working with AI agents

This repo is designed to be built primarily by AI coding agents, and to survive switching between
them mid-project. [`AGENTS.md`](AGENTS.md) is the canonical contract for every agent;
[`CLAUDE.md`](CLAUDE.md) imports it and adds Claude Code mechanics. The durable-memory model —
what gets written where, so context survives a session ending — is
[`docs/agents/CONTEXT_SYSTEM.md`](docs/agents/CONTEXT_SYSTEM.md).

## Renaming

The codename appears in: package names (`diapason_*`), the Rust crate prefix, the bundle ID
(`dev.jfcofer.diapason`, `adr/0013`), and the app display name. `just rename <new_name>
<com.your.bundle>` handles all of them, including the generated iOS project.

## License

[Functional Source License 1.1, Apache-2.0 future licence](LICENSE.md) (`FSL-1.1-ALv2`).

- **Allowed:** read, audit, build, modify and share it for any purpose except offering a competing
  commercial product.
- **Becomes Apache-2.0:** each version converts automatically two years after its release.
- **Why:** [`docs/adr/0017-project-licence.md`](docs/adr/0017-project-licence.md).

**Contributions:** by submitting a contribution you license it to the project's licensor under the
[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0), and agree that it is
distributed as part of this project under the licence above.

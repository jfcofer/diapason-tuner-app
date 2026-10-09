---
id: T-002a
title: Engine real-time core, proven offline
status: done
milestone: M1
owner: claude
created: 2026-10-04
---

## Goal

Build the part of the audio spine that does not depend on any device: the backend contract, the
lock-free command/snapshot plumbing, preallocation, and an `OfflineBackend` that drives it all from
fixture buffers. Every later backend plugs into this, and every RT-safety rule in `AGENTS.md` §6 is
enforced here by tests, on any host, before a microphone is involved.

## Context

Read first: `docs/PLATFORM_AUDIO.md` §1 (backend contract), `docs/AUDIO_ENGINE.md` §7
(RT-safety enforcement), `docs/ARCHITECTURE.md` §4 (threading). Parent: `T-002`.

What exists: `rust/crates/audio_io` has a three-method `AudioBackend` stub (`name`/`start`/`stop`)
that **does not match** the documented contract (`open`/`actual_config`/`timestamp`/`close`).
`engine` has only a static `EngineSnapshot`. No audio crate is a dependency anywhere yet.

Before adding `rtrb`, `triple_buffer` and `assert_no_alloc`, check their current versions and
APIs on crates.io and docs.rs (`AGENTS.md` §3.3), and confirm each passes `cargo deny`.

## Acceptance criteria

- [x] `AudioBackend` in `audio_io` matches `PLATFORM_AUDIO.md` §1. The stub is replaced, not
      extended
- [x] `OfflineBackend` drives the callback from a fixture buffer at a chosen block size, with no
      real-time clock (deterministic)
- [x] `engine`: commands via `rtrb` SPSC, snapshots via `triple_buffer`, and nothing else across
      threads (`AUDIO_ENGINE.md` §7)
- [x] `Engine::prepare(max_block_size, sample_rate)` preallocates every buffer the callback uses
- [x] Payload: input RMS in the snapshot; a test tone on command, phase-continuous across blocks
- [x] A backend conformance suite that any `AudioBackend` can be run through. `OfflineBackend`
      passes it
- [x] Zero-allocation test: 10 s through `OfflineBackend` with `assert_no_alloc` armed; the
      suite fails if the callback allocates (proven by a deliberately allocating test case)
- [x] `clippy::disallowed_methods` lists the `AUDIO_ENGINE.md` §7 methods for `engine`
- [x] Fixture-based accuracy tests for RMS (known sine, known amplitude) and tone frequency
- [x] `just verify` green

## Out of scope

Any platform backend (`T-002b`, `T-002c`). FFI surface changes beyond what the snapshot needs. Any
UI. Pitch detection (`T-003`).

## Implementation notes

- **Contract as built** (`PLATFORM_AUDIO.md` §1 updated to match):
  - `actual_config` returns `Option`, because a closed backend has no config.
  - `close` is idempotent and drops the callback.
  - `StreamHandle` is a set of atomic counters (callbacks, frames, largest block). It is how a
    device harness will know audio is flowing.
  - `CallbackInfo` carries the channel counts.
  - New error variants: `AlreadyOpen`, `NotOpen`, `BufferMismatch`, `InvalidConfig`.
- **Conformance suite** (`audio_io/src/conformance.rs`, under `cfg(test)` or the `conformance`
  feature):
  - It drives backends through a `Harness`: `backend()` + `advance(frames)`. Offline renders;
    devices will wait on the handle.
  - Eight checks. Breaking `close` or the block clamp fails it.
- **DSP** (`dsp::osc`, `dsp::level`): an `f64`-phase sine with a 5 ms level ramp, and an RMS meter
  whose window counts samples. Proptests show both are bit-identical under any block partition.
- **`adr/0018`**, decided with the owner: pedantic flags every `f64 → f32`, and `#[allow]` is
  banned.
  - Lossy casts now live only in `dsp::convert`, under `#[expect]`.
  - `docs-check` rejects any other `#[allow]`/`#[expect]` (Rust) or `// ignore:` (Dart).
- **Engine:**
  - `prepare` returns `(Engine, Processor)`.
  - `EngineSnapshot` is now a `Copy` POD, and the build strings moved to `BuildInfo`. The FFI maps
    from it, and only the Dart doc comments changed.
  - `assert_no_alloc` wraps the callback unconditionally. Its default `disable_release` makes it a
    no-op in release builds, which equals the documented `#[cfg(debug_assertions)]`.
- **The no-alloc canary** has to run in a child process: the trap aborts (`handle_alloc_error`), it
  cannot panic.
  - A clean child must report "1 passed", and the allocating one must die of SIGABRT.
  - The test file is `#![cfg(debug_assertions)]`, since `AllocDisabler` does not exist in release
    builds.
- **The clippy list** lives in `engine/clippy.toml`, with `dsp/clippy.toml` symlinked to it,
  because the RT path runs through `dsp`.
  - Its first run caught `format!` in `dsp::build_id`, and in proptest's `prop_assert_eq!`
    expansion.
  - **Trap:** editing `clippy.toml` does not invalidate clippy's cache. `touch` a source file
    before trusting a clean run.
- **Licences:** `deny.toml` now allows MPL-2.0 (`triple_buffer`) and BSD-1-Clause
  (`assert_no_alloc`), as `adr/0009` already permitted.
- **Not done, by design:**
  - The RT log ring (nothing logs yet).
  - Installing `AllocDisabler` as the global allocator in debug *app* builds, so that the device
    traps too. That belongs to `T-002b`.
  - `rtsan-standalone`, which would also catch locks and syscalls, but needs a compiler-rt build.

**Adversarial review** (the `reviewer` subagent) found two must-fix gaps, both fixed:

- **Sample-rate drift.** The engine was tuned to the *requested* rate. A device granting
  44.1 kHz would have put the tone about 147 cents flat.
  - Fix: `CallbackInfo` carries the granted rate, and the processor rebuilds its rate-dependent
    state to follow it, without allocating.
- **A clock contract with no teeth.** The stream clock is now contiguous from 0, host time rises
  with every block, and `timestamp()` is the latest block's stamp.
  - A `Faulty` backend injects each of four violations, and the suite rejects every one.

Also from the review:
- The snapshot carries the real stream clock.
- The command drain is bounded.
- `docs-check` now catches `cfg_attr` allows and Cargo `[lints]` allows.
- Docs no longer claim the debug app traps allocations.

Left open: `adr/0018` puts lossy casts in `dsp::convert`, which `audio_io` cannot reach. This has
to be resolved before `T-002b` (see STATE).

## Verification performed

Fedora 44, Rust 1.98.0. Each guard was shown to fail before it was trusted. Every mutation below
was temporary and reverted.

| Guard | Mutation | Result |
|---|---|---|
| Conformance suite | `close` keeps the stream | `close kept the callback alive` |
| Conformance suite | block clamp removed | `block of 2048 frames exceeds the granted maximum 512` |
| Phase-continuity proptest | phase reset on every `fill` | fails |
| Canary | callback stops allocating | `an allocating callback was not trapped` |
| Zero-alloc test | `vec!` in `Processor::render` | `SIGABRT`, `memory allocation of 1 bytes failed` |
| `clippy.toml` | `Vec::push` + `Instant::now` in the processor | both rejected |
| `docs-check` | `#[allow]`, `cfg_attr(…allow)`, Cargo `"allow"`, Dart `// ignore:` | all rejected |
| Rate following | `follow_sample_rate` disabled | `the_engine_follows_the_rate_the_device_grants` fails |
| Conformance suite | stuck `timestamp()`, end stamps, frozen host clock, wrong rate | each rejected |

- **Accuracy:**
  - Tone within 1 mHz of target at 44.1 and 48 kHz, also through the engine with irregular blocks.
  - RMS within 1e-6 relative of A/√2, also through the engine.
  - Irregular-block output is bit-identical to fixed-block output.
- **Zero allocation:** 10 s at 48 kHz (480 000 frames) through `OfflineBackend`, blocks cycling
  `[1, 17, 96, 192, 511, 512, 3]`, three commands mid-run, trap armed around every render. It
  passes.

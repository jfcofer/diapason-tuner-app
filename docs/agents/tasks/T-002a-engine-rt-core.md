---
id: T-002a
title: Engine real-time core, proven offline
status: todo
milestone: M1
owner: unassigned
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

- [ ] `AudioBackend` in `audio_io` matches `PLATFORM_AUDIO.md` §1. The stub is replaced, not
      extended
- [ ] `OfflineBackend` drives the callback from a fixture buffer at a chosen block size, with no
      real-time clock (deterministic)
- [ ] `engine`: commands via `rtrb` SPSC, snapshots via `triple_buffer`, and nothing else across
      threads (`AUDIO_ENGINE.md` §7)
- [ ] `Engine::prepare(max_block_size, sample_rate)` preallocates every buffer the callback uses
- [ ] Payload: input RMS in the snapshot; a test tone on command, phase-continuous across blocks
- [ ] A backend conformance suite that any `AudioBackend` can be run through. `OfflineBackend`
      passes it
- [ ] Zero-allocation test: 10 s through `OfflineBackend` with `assert_no_alloc` armed; the
      suite fails if the callback allocates (proven by a deliberately allocating test case)
- [ ] `clippy::disallowed_methods` lists the `AUDIO_ENGINE.md` §7 methods for `engine`
- [ ] Fixture-based accuracy tests for RMS (known sine, known amplitude) and tone frequency
- [ ] `just verify` green

## Out of scope

Any platform backend (`T-002b`, `T-002c`). FFI surface changes beyond what the snapshot needs. Any
UI. Pitch detection (`T-003`).

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._

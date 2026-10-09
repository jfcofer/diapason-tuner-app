# 2026-10-09 — engine-rt-core

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002a  **Milestone:** M1

## Done

- **`audio_io`:**
  - the backend contract (`PLATFORM_AUDIO.md` §1);
  - `OfflineBackend`, with fixed and cycled block patterns on a synthetic clock;
  - a conformance suite run through a `Harness`, itself tested against a `Faulty` backend.
- **`dsp`:** a phase-continuous sine with a 5 ms level ramp, a block-invariant RMS meter, and
  `convert` (`adr/0018`).
- **`engine`:**
  - `Engine::prepare` → `(Engine, Processor)`, with `rtrb` commands and `triple_buffer` snapshots;
  - it follows the granted sample rate;
  - a zero-allocation test with a SIGABRT canary;
  - `clippy.toml` RT bans.
- **`adr/0018`**, decided with the owner. `docs-check` now rejects lint suppressions.
- The `reviewer` subagent's two must-fix findings (rate drift, a weak clock contract) were fixed
  on the same branch.

## Tried and abandoned

- **`#[should_panic]` for the allocation canary.** `assert_no_alloc` aborts, because panicking
  allocates, so the canary re-runs its own test binary as a child instead. A clean control child
  rules out false passes.
- **Asking backends to honour the requested sample rate.** `PLATFORM_AUDIO.md` §2 says to use the
  device's native rate, so the engine follows the granted one instead.
- **Emulating a stuck `timestamp()` by latching the first *call*.** It latched the latest block
  and proved nothing. A real fault latches the first *block*.

## Surprises

- **Every DSP line collides with the rules:** pedantic flags `f64 → f32`, and `#[allow]` is
  banned. Hence `adr/0018`.
- `triple_buffer` is MPL-2.0 and `assert_no_alloc` is BSD-1-Clause. `adr/0009` allowed both, but
  `deny.toml` did not.
- The clippy RT list caught `format!` in `dsp::build_id` and inside `proptest`'s assertion macros.
  Clippy's cache had hidden it until a source file was touched.
- **A doctest failure was committed** because two commands were chained with `;`. It was caught
  on the next run and fixed before any push.

## Left for next session

- Push and open PRs for `chore/T-006-close-m0` and `feat/T-002a-engine-rt-core`, in that order.
  This needs the owner's approval.
- The `adr/0018` question for `audio_io` (STATE.md, open questions), then `T-002b`.

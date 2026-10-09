# 0018 — Lossy numeric conversions live in one audited module

**Status:** Accepted · 2026-10-09

## Context

`clippy::pedantic` is required on `dsp` (`AGENTS.md` §7). Its cast lints
(`cast_possible_truncation`, `cast_precision_loss`, `cast_sign_loss`) flag every narrowing `as`,
including `f64 → f32`. Rust has no lossless form of that conversion. Audio samples are `f32` by
contract (`PLATFORM_AUDIO.md` §1), and accurate DSP keeps phase and sums in `f64`. So the
narrowing is intrinsic to the domain, not a defect.

`AGENTS.md` §3.2 forbids `#[allow]` to get the gate green: "fix the cause or escalate". For these
casts there is no cause to fix. `T-002a` met this with its first oscillator, and `T-003` (pitch
detection) will meet it on every other line.

## Decision

- Every lossy numeric conversion in Rust goes through **`rust/crates/dsp/src/convert.rs`**: a few
  small named functions (such as `to_sample(f64) -> f32`), each documenting why its loss is
  acceptable.
- Each function there carries a scoped `#[expect(clippy::…, reason = "…")]`, never `#[allow]`.
  `#[expect]` fails the build when the lint stops firing, so a stale exception cannot linger.
- **`docs-check` fails** on `#[allow(`, `#![allow(` or `#[expect(` in any hand-written Rust file
  other than `convert.rs`. It also fails on `// ignore:` or `// ignore_for_file:` in hand-written
  Dart. Generated code is exempt: FRB output, `*.g.dart`, `*.freezed.dart`, and vendored cargokit.
- `engine` and later crates call `diapason_dsp::convert`. They do not grow their own.

## Consequences

**Good.**
- The `AGENTS.md` §3.2 rule goes from a convention to a check.
- Every lossy conversion in the project can be audited in one file.
- A reviewer sees a new exception as a diff to `convert.rs`, not as an attribute buried in a
  function.

**Bad.** One extra indirection at each conversion site. A conversion that is lossy in a new way
needs a new function and its justification. That cost is intended.

**Rejected.**
- *Allowing the cast lints in `dsp`'s `[lints]` table*: it silently permits every narrowing
  anywhere, so the lint stops protecting anything.
- *Cast-free contortions* (integer phase split into `u16` halves, `try_from` on sample rates,
  `f32`-only paths): less precise, harder to read, and worse with every algorithm `T-003` adds.

# 0015 — Pin the Rust toolchain to an exact release

**Status:** Accepted · 2026-10-04

## Context

`rust-toolchain.toml` said `channel = "stable"`. Every other toolchain in this repo is pinned in
`tools/versions.env` and enforced by `just doctor` (`adr/0011`), but Rust floated. `Cargo.toml`'s
`rust-version = "1.98"` is a *minimum*, not a pin: it stops an older compiler, not a newer one.

That gap matters because the gate runs `cargo clippy -- -D warnings` and `cargo fmt --check`. Each
Rust release adds lints and can change formatting. With a floating channel, CI could turn red with
no code change, and a developer's machine and CI could disagree about the same commit. When that
happens, the fix someone reaches for is an `#[allow]`, which `AGENTS.md` §3.2 forbids.

## Decision

- `rust-toolchain.toml` pins `channel` to an exact release, currently **1.98.0**, the version
  `T-001` was verified on.
- `tools/versions.env` carries `RUST_VERSION`. `just doctor rust` fails if the channel and the pin
  disagree, or if the active `rustc` is not that version. `just doctor-selftest` proves both checks.
- CI installs the toolchain with `rustup toolchain install`, which reads `rust-toolchain.toml`, so
  CI cannot use a different compiler from a developer.
- `rust-version` in `Cargo.toml` stays the MSRV and moves with the pin.

## Consequences

**Good.** Compiler-driven red builds become deliberate events. Rust joins every other toolchain in
being reproducible from `versions.env`.

**Bad.** New Rust releases do not arrive on their own. Bumping is a small task of its own: change
`RUST_VERSION`, `channel` and `rust-version`, then fix whatever new lints report. Plan to do this
roughly once a quarter, not every six weeks.

**Rejected.** A floating `stable` with a scheduled CI run to catch breakage early: it detects drift
but does not prevent it, and it leaves local and CI compilers able to differ.

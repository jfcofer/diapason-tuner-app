# 0007 — Note and temperament maths exists in both Rust and Dart

**Status:** Accepted · 2026-08-25

## Context

Converting between frequency, note name, cents and temperament tables is needed in two places:
inside `dsp`, at analysis rate, on the RT-adjacent path; and inside the Flutter UI, for display,
settings previews and the instrument picker. The obvious instinct is to implement it once in Rust
and expose it over FFI.

## Decision

Implement it in both, and pin both to `fixtures/note_table.json`, which each test suite asserts
against.

## Consequences

**Good.** The UI does not take an FFI round trip to render a settings preview or a string list, and
`core_domain` stays pure Dart — trivially testable, no native build needed for UI tests. The Rust
side stays free of FFI-shaped compromises in its hot path. Neither can drift, because the shared
fixture fails loudly if they disagree.

**Bad.** It is duplication, and it looks like a mistake to anyone who has not read this ADR — which
is precisely why the ADR exists, and why both implementations carry a comment pointing here. A new
temperament must be added in two places plus the fixture.

**Rejected: single implementation over FFI.** Couples every UI interaction to the engine's
availability and lifecycle, forces a native build for UI tests, and puts a synchronous FFI call in
the path of a settings slider.

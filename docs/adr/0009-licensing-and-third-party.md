# 0009 — Licensing and dependency policy

**Status:** Proposed · 2026-08-25 — decide before M7

## Context

Two open questions with real consequences: the licence of this project, and what we accept from
dependencies. The app bundles Rust crates, Dart packages, Google's Oboe (Apache-2.0), and fonts.
Fonts in particular are a common shipping violation — bundling a face whose licence does not permit
app embedding.

## Decision (proposed)

- Project licence: **to be decided** before first public release.
- Dependency policy, effective now: permissive licences only (MIT, Apache-2.0, BSD, ISC, Zlib,
  MPL-2.0 for unmodified libraries). No GPL or LGPL in the shipped artifact. Enforced by
  `cargo deny check licenses` in CI and a Dart licence audit in `just verify`.
- Fonts must be OFL or equivalent with explicit embedding permission, and the licence file ships in
  the app's about screen.
- Every new dependency needs a one-line justification in the PR. An audio app's dependency tree
  should be embarrassingly small; each addition is a maintenance and a supply-chain liability.

## Consequences

**Good.** No licence surprise a week before submission. `cargo deny` also catches advisories and
duplicate versions, which is worth having independently.

**Bad.** Occasionally rules out a convenient package. That is the intended trade.

**Open:** the project licence itself, which depends on the monetisation decision in
`PRODUCT_SPEC.md` §6. Supersede this ADR when both are settled.

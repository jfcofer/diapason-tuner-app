---
id: T-005
title: Choose the project licence; protect main
status: done
milestone: M0
owner: claude
created: 2026-10-04
---

## Goal

The repository is public with no licence, which means it is all-rights-reserved by default, and
`main` has no protection. Decide and apply the licence, and enforce the merge workflow that
`AGENTS.md` §3 describes.

## Context

`adr/0009` (dependency policy, licence open), `PRODUCT_SPEC.md` §6 (paid app, no ads),
`adr/0010` (no telemetry).

## Acceptance criteria

- [x] Licence decided by ADR (`adr/0017`): FSL-1.1-ALv2, the official template text in
      `LICENSE.md`, and `license` set on every crate
- [x] `adr/0009` moved from Proposed to Accepted (dependency policy), pointing at `adr/0017`
- [x] Contribution terms stated (inbound Apache-2.0, outbound FSL) in `README.md`
- [x] Repo merge settings: rebase only, head branches deleted on merge
- [x] Ruleset on the default branch, with no bypass actors: PR required; the six CI checks
      required on an up-to-date branch; linear history; no force-push; no deletion
- [x] `AGENTS.md` states the merge policy
- [x] `just verify` green; this change lands through the protected flow itself

## Out of scope

Monetisation (M6). An in-app licences screen for dependencies and fonts (`adr/0009`, M7).

## Implementation notes

- **Licensor.** The licensor is the git identity `jfcofer`. The owner may replace it with their
  legal name in `LICENSE.md`; that is a one-line change and needs no ADR.
- **Why no bypass.** Agents act through the owner's GitHub credentials, so an admin bypass would
  let an agent push to `main` directly.
- **Ruleset id.** `24468437`, at `gh api repos/jfcofer/diapason-tuner-app/rulesets/24468437`.

## Verification performed

- `gh api .../rules/branches/main` lists: deletion, non_fast_forward, required_linear_history,
  pull_request and required_status_checks.
- `cargo metadata` reports `FSL-1.1-ALv2` for all five crates.
- `just verify` is green.
- This PR is the first change merged under the ruleset.

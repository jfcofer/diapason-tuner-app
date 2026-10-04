# 0017 — Project licence: FSL-1.1-ALv2

**Status:** Accepted · 2026-10-04

## Context

`adr/0009` left the project licence open until monetisation was settled. Two things have changed
since then. The repository is now **public**, and with no licence a public repository is
all-rights-reserved by default: people can read the code but have no right to use it. The product
direction is also firmer. `PRODUCT_SPEC.md` §6 assumes a one-time purchase or a paid unlock, with
no ads and no data collection under any model.

What the licence has to do for this project:

1. **Protect a paid app.** Under a permissive licence (MIT, Apache-2.0), anyone could build this
   repository and publish it on the stores for free, competing with the paid version.
2. **Make the privacy claim verifiable.** "Zero telemetry, zero network requests" (`adr/0010`) is
   strongest when anyone can read the code that proves it.
3. **Stay compatible with the dependency policy** in `adr/0009` (permissive dependencies only).
4. **Be reversible in the right direction.** A licence can always be relaxed later, but code
   already published permissively can never be pulled back.

## Decision

The project is licensed under the **Functional Source License 1.1, Apache-2.0 future licence**
(SPDX `FSL-1.1-ALv2`). The text in `LICENSE.md` is the official template from
github.com/getsentry/fsl.software, with only the year and licensor filled in.

- **Allowed:** anyone may read, use, modify and redistribute the code for any purpose except a
  *Competing Use*, meaning a commercial product that substitutes for this app. Internal use,
  non-commercial education and research are explicitly permitted.
- **Future licence:** each version automatically becomes **Apache-2.0** two years after it is
  published. The project ends up fully open source, on a delay.
- **Rust metadata:** `license = "FSL-1.1-ALv2"` is set in `[workspace.package]`, and every crate
  inherits it.
- **Contributions:** they are licensed to the licensor under Apache-2.0 (inbound), and the
  project is distributed under FSL (outbound). This lets the licensor ship contributed code in the
  paid app without a CLA. The terms are stated in `README.md`.

## Consequences

**Good.** The revenue model is protected while the full source stays public, so the privacy claim
can be audited. The licence is a standard, SPDX-listed template, not a bespoke one. Because every
version converts to Apache-2.0, the long-term outcome is real open source. The dependency policy
is unaffected, since FSL is the licence of *this* code, not a requirement on dependencies.

**Bad.** FSL is **not OSI-approved**, so this is "source-available", not "open source", for two
years per version. F-Droid will not list it; a paid app would not be there anyway. Some developers
will not contribute to non-OSI projects.

**Rejected.**
- **MIT or Apache-2.0 now:** permits free store clones of a paid app, and cannot be undone.
- **GPL-3.0 or AGPL-3.0:** these permit commercial clones as long as the source is published.
  Once outside contributions arrive, GPL also needs a CLA for the owner's own App Store
  distribution.
- **PolyForm Noncommercial:** it forbids *all* commercial use and has no conversion to open
  source.
- **BSL 1.1:** it does the same job with a per-project change date and more parameters to get
  wrong. FSL is the simpler descendant of the same idea.

This ADR is not legal advice. If the app reaches meaningful revenue, a lawyer should confirm the
licence and the contribution terms.

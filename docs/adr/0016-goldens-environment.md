# 0016 — Goldens run on the pinned Ubuntu runner, not a third-party container

**Status:** Accepted · 2026-10-04

## Context

Golden images are platform-specific, because font rasterisation and anti-aliasing differ between
operating systems. `docs/TESTING.md` §3 therefore said goldens run "in a pinned Linux container",
and CI used `ghcr.io/cirruslabs/flutter:<FLUTTER_VERSION>`.

The first CI run showed that image does not exist: `manifest unknown`. The registry's tag list
stops at **3.44.0**, and the repo pins Flutter **3.47.1** (`adr/0011`). Pinning Flutter to whatever
a third party last published would reverse the order of authority: the toolchain pin would follow
the container, instead of the container following the pin.

No golden tests exist yet (`core_ui` ships widgets in M3), so `tools/goldens.sh` passes with
nothing to compare.

## Decision

- Goldens run in CI on the **`ubuntu-24.04` runner image**, with Flutter installed at the pin by
  `.github/actions/toolchain`, the same way every other job gets it. No third-party container.
- Golden tests load the app's **bundled fonts only**, never system fonts, so the image's font
  packages cannot change a pixel.
- How goldens are *regenerated* is decided with the first golden test in M3. Until then,
  `just goldens-update` refuses to run and says so. The two candidates are a CI job that uploads
  regenerated images as an artifact, and a repo-owned container image built from a Dockerfile at
  the pin. That M3 decision supersedes this paragraph with its own ADR.

## Consequences

**Good.** The Flutter pin stays the single authority. The goldens job uses the same setup path as
`dart` and `checks`, so there is nothing extra to maintain.

**Bad.** A runner image update (`ubuntu-24.04` revisions) could in principle shift anti-aliasing.
Bundled fonts remove the largest source of drift but not all of it, and M3 must choose a comparator
tolerance with that in mind.

**Rejected.** Pinning to the last cirruslabs tag (3.44.0): it would put goldens on a different
Flutter from the one the app ships with, which is worse than having no goldens.

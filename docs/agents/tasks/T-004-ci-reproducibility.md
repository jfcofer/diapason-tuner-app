---
id: T-004
title: Close the remaining CI reproducibility gaps
status: todo
milestone: M1
owner: unassigned
created: 2026-10-04
---

## Goal

After `T-001`, every toolchain pin is enforced except a handful of CI tools and the Xcode version.
Close those, so a red build always means a code change. It must never mean an upstream release.
This continues `adr/0015`'s argument; it is not a new direction.

## Context

These came from the adversarial review of the `T-001` CI work (journal `2026-10-04-ci-green`).
Each one is real; none blocks `T-002`. Read `docs/CI_RELEASE.md` §1 and
`.github/actions/toolchain/action.yml` first.

## Acceptance criteria

- [ ] `just`, `cargo-nextest`, `cargo-deny`, `shellcheck` and `cargo-ndk` (used by
      `just test-android-device`; 4.1.2 locally) are installed at versions pinned in
      `tools/versions.env`. Check that `taiki-e/install-action` accepts `tool@version` before
      relying on it. `doctor` compares versions, not just presence, and the selftest proves it
- [ ] Xcode is pinned on `build-ios` (`xcode-select` to a version in `versions.env` that exists on
      the `macos-26` image), and `DEVELOPMENT.md` §1 says so
- [ ] The composite action's `run:` blocks are linted. actionlint does not cover `action.yml`
- [ ] After `build-ios-unsigned`, CI asserts that Flutter's macOS-side migrations did not rewrite
      what `tools/ios/configure_project.rb` generates (`git diff --exit-code` on those paths)
- [ ] Decide whether `Podfile.lock` is committed (CocoaPods advises it for apps) or ignored, and
      make `.gitignore` say so
- [ ] `check-android-release` reads targetSdk/minSdk from the AAB itself
      (`bundletool dump manifest`), not from `versions.env`
- [ ] `configure_project.rb` removes the configurations, schemes and xcconfigs of a flavour that
      was dropped from `FLAVOURS`
- [ ] `just verify` green; CI green

## Out of scope

Bumping any existing pin. Goldens tagging, which belongs to M3 and is noted in `adr/0016`. The
plugin's SwiftPM support, which is its own task: see STATE.md.

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._

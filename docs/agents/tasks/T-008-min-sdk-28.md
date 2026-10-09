---
id: T-008
title: Raise Android minSdk from 26 to 28
status: in-progress
milestone: M1
owner: claude
created: 2026-10-09
---

## Goal

Make API 28 (Android 9) the floor, so that every supported device can open the microphone with the
`Unprocessed` or `VoiceRecognition` preset. That preset decides whether the tuner hears the
instrument or an AGC-flattened, voice-filtered version of it (`PLATFORM_AUDIO.md` §2). This
unblocks the `T-002b` binding.

## Context

The owner decided this on 2026-10-09, while planning `T-002b`. `AGENTS.md` §8 makes a minSdk
change its own task and its own ADR. The evidence:

- The `oboe` crate is unmaintained: 0.6.1 on 2024-03-03 and no commits since. The `ndk` crate is
  maintained and is what cpal 0.18 uses on Android, so the binding is AAudio via `ndk`.
- `ndk` exposes `input_preset`, `usage` and `content_type` only with its `api-level-28` feature.
  Bionic binds symbols at load time, so linking them would make the `.so` fail to load on 8.x.
  Keeping 26 would mean resolving them with `dlsym`: more `unsafe`, and a degraded tuner on 8.x.
- Oboe itself avoids AAudio on 8.0 as unreliable.
- StatCounter, June–July 2026: "8.0 Oreo" (8.0 and 8.1) at about 4–8% regionally (Oceania 4–7%,
  Germany 5.9%). There is no worldwide figure; re-check before writing the ADR.

Read: `tools/versions.env`, `docs/PLATFORM_AUDIO.md` §2, `docs/DEVELOPMENT.md` §1,
`tools/check-android-release.sh`. `build.gradle.kts` already reads the pin.

## Acceptance criteria

- [ ] `docs/adr/0019-min-sdk-28.md`: evidence, decision, rejected options (keep 26 with
      `dlsym`; 27 with `dlsym`), and the cost in reach
- [ ] `ANDROID_MIN_SDK=28` in `tools/versions.env`, the only place the number is written
- [ ] `PLATFORM_AUDIO.md` §2 and `DEVELOPMENT.md` §1 updated; `T-002b`'s Context no longer says 26
- [ ] `just check-android-release` on a built release APK reports `minSdk 28`
- [ ] `just verify` green; `build-android` CI green

## Out of scope

The audio binding itself (`T-002b`). `targetSdk`, which stays 36.

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._

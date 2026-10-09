---
id: T-008
title: Raise Android minSdk from 26 to 28
status: done
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

- [x] `docs/adr/0019-min-sdk-28.md`: evidence, decision, rejected options (keep 26 with
      `dlsym`; 27 with `dlsym`), and the cost in reach
- [x] `ANDROID_MIN_SDK=28` in `tools/versions.env`, the only place the number is written
- [x] `PLATFORM_AUDIO.md` §2 and `DEVELOPMENT.md` §1 updated; `T-002b`'s Context no longer says 26
- [x] `just check-android-release` on a built release APK reports `minSdk 28`
- [x] `just verify` green; `build-android` CI green

## Out of scope

The audio binding itself (`T-002b`). `targetSdk`, which stays 36.

## Implementation notes

- **Reach evidence:** StatCounter, September 2026, worldwide mobile and tablet. Android 11–16 are
  91.16%, so Android 10 *and older*, together, are 8.84%. The page does not break out 8.x.
- **Oboe's rule,** read from its source: `isAAudioRecommended` requires `__ANDROID_API_O_MR1__`
  (27). Its comment calls AAudio on 8.0 "error prone".
- **Corrected while writing the ADR:** a first draft said OpenSL ES cannot request an unprocessed
  source on 26–27. It can (`SL_ANDROID_RECORDING_PRESET_UNPROCESSED`), and the ADR says so. Oboe
  was rejected on maintenance, `libc++_shared` and the cost of verifying a second API, not on that
  claim.
- `T-002b`'s Context was updated on the `T-007` branch.

## Verification performed

- `flutter build apk --release --flavor prod --target-platform android-arm64` with
  `CARGO_BUILD_JOBS=4`: built in 52 s, 18.2 MB.
- `tools/check-android-release.sh` on that APK reported:
  - `minSdk 28`, `targetSdk 36`;
  - 16 KB `LOAD` alignment on arm64-v8a and x86_64 `libdiapason_ffi.so`;
  - 17 MB of the 60 MB budget.
- The AAB that CI builds goes through the same checks in `build-android`.
- **Merged as #9** on 2026-10-09, rebase-merged with all six CI jobs green, including
  `build-android`.

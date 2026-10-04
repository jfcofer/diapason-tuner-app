---
id: T-002c
title: iOS CoreAudio backend, verified as far as CI can reach
status: todo
milestone: M1
owner: unassigned
created: 2026-10-04
---

## Goal

The iOS half of the audio spine: a CoreAudio (AURemoteIO) backend through the `T-002a` engine, plus
the Swift-side `AVAudioSession` setup, verified as far as this project's hardware allows. That
hardware is no Mac and no iPhone, only the `macos-26` CI runner and its **iOS Simulator**.

## Context

Read first: `docs/PLATFORM_AUDIO.md` (the iOS sections). Parent: `T-002`. Depends on `T-002a`.

The iOS Simulator runs real CoreAudio code paths against the host's audio, so it is the closest
thing to running this code that the project can get. It does not stand in for a device. Latency,
interruptions from real calls, and Bluetooth routing are **unverifiable** here, and STATE.md must
say so rather than tick them.

**Spike first, before trusting the criteria below.** It is unverified whether a headless `macos-26`
runner's Simulator exposes an audio input, and whether mic permission can be granted
non-interactively (`xcrun simctl privacy`). If either fails, amend the Simulator criteria before
building toward them.

Pick the CoreAudio binding on evidence, as `T-002b` does for Android: a crate vs a thin
hand-written FFI over AudioUnit. Whatever is chosen lives in `audio_io`, with `// SAFETY:` on every
`unsafe` block.

## Acceptance criteria

- [ ] Binding decision recorded (ADR if it closes off an alternative)
- [ ] The CoreAudio backend compiles for `aarch64-apple-ios` and `aarch64-apple-ios-sim` in CI
- [ ] `AVAudioSession` configured from Swift per `PLATFORM_AUDIO.md` (`.playAndRecord`,
      `.measurement` while the tuner runs)
- [ ] A CI job boots an iOS Simulator and runs an integration test that opens the duplex stream,
      sees the snapshot update, and closes it cleanly
- [ ] The backend passes the `T-002a` conformance suite on the Simulator
- [ ] Microphone permission flow implemented behind `MicrophonePermission`. The Simulator covers
      grant and deny
- [ ] Unverifiable criteria (device latency, call interruption, Bluetooth, lock screen) listed in
      STATE.md as open, not checked
- [ ] `just verify` green; `build-ios` CI green

## Out of scope

Android. Pitch detection. On-device verification, which waits for an iPhone.

## Implementation notes

_Fill in during the work._

## Verification performed

_Fill in during the work._

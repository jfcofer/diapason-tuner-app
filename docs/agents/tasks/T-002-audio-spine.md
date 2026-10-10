---
id: T-002
title: Prove one duplex audio stream on both platforms
status: in-progress
milestone: M1
owner: claude
created: 2026-08-25
---

## Goal

Get real audio in and out on real devices, through the real backends, with the smallest possible
payload: publish input RMS in the snapshot and render a test tone on request. This retires the
single largest technical risk in the project — everything after it is comparatively predictable.

## Context

Read first: `docs/PLATFORM_AUDIO.md` in full, `docs/ARCHITECTURE.md` §4 (threading),
`docs/AUDIO_ENGINE.md` §7 (RT-safety enforcement). You do not need the pitch-detection sections yet.

This task is where platform reality asserts itself. Expect: Android devices that refuse the fast
path, an input preset that silently falls back and quietly ruins accuracy later, iOS sessions that
behave differently under `.measurement`, and a simulator that lies about latency. Measure and report
rather than assume — the diagnostics overlay built here is what makes every later audio bug
diagnosable.

## Acceptance criteria

- [ ] `AudioBackend` trait with `AAudio` (amended from `Oboe`: `adr/0020`), `CoreAudio` and
      `Offline` implementations; `Cpal` may be stubbed
- [ ] A shared backend conformance test suite that every backend passes
- [ ] Duplex stream: microphone in and output out simultaneously, one clock
- [ ] Microphone permission flow on both platforms, including denial and revocation-while-running,
      with the metronome path unaffected by denial
- [ ] Diagnostics overlay reports: actual sample rate, buffer size, measured round-trip latency,
      callback worst-case duration, xrun count, and (Android) the input preset actually obtained
- [ ] Every row of the lifecycle matrix in `PLATFORM_AUDIO.md` §5 behaves as specified, verified on
      device, with an integration test where automatable
- [x] Zero-allocation test passes: 10 s through `OfflineBackend` with `assert_no_alloc` armed
- [ ] Stream rebuilds cleanly on device disconnect and route change, off the RT thread
- [ ] Measured latency recorded in the journal for every reference device available (today: the
      Redmi 23117RA68G only - see Implementation notes)

## Out of scope

Pitch detection. Metronome scheduling. Any real UI. Latency *calibration* UI (M5) — measurement
only here.

## Implementation notes

**Split into slices (2026-10-04)**, each verifiable on its own with the hardware that exists: a
Linux host, one budget Android (Redmi 23117RA68G), an emulator, and macOS CI runners. There is no
Mac and no iPhone.

- `T-002a` — engine RT core, `OfflineBackend`, conformance suite, zero-alloc test. Host-only.
- `T-002b` — Android duplex on the Redmi, binding ADR first (`oboe` vs `ndk` AAudio).
- `T-002c` — iOS CoreAudio, verified on the CI Simulator. On-device rows stay open.

This task is done when all three are, plus whatever the slices could not reach is listed as open
in STATE.md. **Criterion amended before work started:** "all four reference devices" became "every
reference device available". Four devices cannot be measured with one, and a criterion nobody can
meet only gets ticked dishonestly or ignored. **Amended 2026-10-09 (`T-010`):** the Android backend
is AAudio over `ndk-sys`, not Oboe (`adr/0020`).

## Verification performed

- **Zero allocation** (`T-002a`): `rust/crates/engine/tests/no_alloc.rs`,
  `ten_seconds_of_audio_never_allocate`, runs in `just test-rust`. Its canary proves the trap fires.

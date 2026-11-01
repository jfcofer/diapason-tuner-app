---
id: T-030
title: Stream readings to Dart, and keep the fake engine honest
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

Readings reach Dart at 60 Hz on their own stream, without pushing the 28-field session DTO to that
rate, and the contract binding the fake and real engines covers them.

## Context

Read: `docs/adr/0003`, `docs/adr/0021`, `docs/adr/0028`, `docs/ARCHITECTURE.md` §5,
`packages/audio_engine/rust/src/api/`, `packages/audio_engine/lib/src/{engine_facade,fake_engine}.dart`,
`packages/audio_engine/lib/src/testing/engine_contract.dart`, `core_domain`'s `SessionSnapshot`.

- **A scalar-only `TunerReadingDto` on its own StreamSink.** `SessionPanel` must never rebuild at 60 Hz.
- **frb's cost at 60 Hz is unmeasured.** Measure it in a profile build; do not assume it.
- **The DTO today drops `clock`.** Readings carry the clock pair: `host_time_ns` is speaker
  presentation, not capture, so a consumer uses only its slope (`T-032`).
- Regenerate with `just gen`; frb bindings are never hand-edited (`AGENTS.md` §8).
- The FFI `api` module's panic denials bind this code (`adr/0021`).

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green; `check-drift` clean after `just gen`.
- [ ] `verifyEngineContract` adds, for both engines: no reading without the microphone, none after
      stop, and `end` rising within an epoch. The fake gains `emitReading`.
- [ ] `just test-integration-android` runs the contract on the Redmi, with a device-only `hearsTone`
      (speaker to microphone) if it proves stable, and a recorded reason if it does not.
- [ ] Profile-build cost of the stream on the Redmi, recorded: what one reading costs Dart per second.
- [ ] A new ADR, `adr/0038`, records the hot stream and adds the ledger's missing rows (ring wait,
      thread wake, frb, Flutter). It refines `adr/0028` by superseding the part it changes.

## Out of scope

Note math and anything on screen (`T-031`). The strobe (`T-032`).

## Implementation notes

## Verification performed

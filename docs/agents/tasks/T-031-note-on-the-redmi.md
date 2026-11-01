---
id: T-031
title: A real note on the Redmi: the listening policy, and text
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

The moment the project has been building towards: play a string, and the right note and cents appear
on the phone, with no tap after the first run. Text is deliberate. This proves the path and measures
it; the strobe follows (`T-032`).

## Context

Read: `docs/adr/0036`, `docs/PRODUCT_SPEC.md` §2, `docs/PLATFORM_AUDIO.md` §5,
`packages/feature_tuner/lib/src/{listening_policy,tuner_controller,tuner_screen}.dart`,
`apps/diapason/patrol_test/lifecycle_test.dart`. Needs `T-003d`, `T-030` and the Redmi.

- `engineTarget` does not change. `listenWanted` starts from a stored consent, and a pause holds for
  the run (`adr/0036`). Consent storage goes through a `core_platform` interface.
- **Riverpod carries discrete state only** (the note, locked or not). Cents go through a
  `ValueListenable` (`ARCHITECTURE.md` §5). No `setState` in feature code.
- **Stage stamps:** log each stage's stream position and host time, dev flavour only, so the budget in
  `AUDIO_ENGINE.md` §1 can be read off a real run. Nothing here may reach the network or the prod build.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] The first run explains and asks; later launches listen with no tap; a pause holds across
      backgrounding in one run; a new launch listens; a denial never re-prompts. Tested on the policy.
- [ ] L11 reruns on the Redmi with its patrol test updated, and `PLATFORM_AUDIO.md` §5 cites the run.
      L1, L2 and L8 are rerun, as their start conditions moved.
- [ ] On the Redmi: each open string of a guitar shows the right note, with cents against a reference
      tuner or tone. Recorded in the journal with the device, the OS and the date.
- [ ] **End-to-end latency per stage**, measured against the rows of §1 (≤ 85, 110, 190 ms), and recorded.
- [ ] If a row misses: first post readings from the analysis thread (up to 16.7 ms saved), then bring
      the owner the measured numbers. **Budgets do not move without the owner.**
- [ ] The dev-only readout `SessionPanel` keeps working in dev and stg.

## Out of scope

The strobe, tokens and final layout (`T-032`, `T-033`). Temperaments and presets.

## Implementation notes

## Verification performed

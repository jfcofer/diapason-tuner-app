---
id: T-002e
title: Diagnostics overlay, and the first golden test
status: todo
milestone: M1
owner: unassigned
created: 2026-10-20
---

## Goal

M1's exit asks for the stream's facts to be visible on device: "measured round-trip latency
reported in the diagnostics overlay". Today they are text lines in the dev-only `SessionPanel`
scaffolding. Build the overlay as a real `core_ui` component, and land the project's first golden
with it, so the golden pipeline (`adr/0016`) is proven before M3 depends on it.

## Context

Read: `docs/DESIGN_SYSTEM.md` (tokens, type), `docs/TESTING.md` §1 and §3, `docs/adr/0016`,
`docs/ARCHITECTURE.md` §5 (snapshots, ~30 Hz). A slice of `T-002`, split out of `T-002b` part 3.
The `flutter-ui` agent's scope.

- The data already reaches Dart in `core_domain`'s `SessionSnapshot`: sample rate, burst, buffer
  and its capacity, xruns, input underruns, frames shed, `callbackLoad`, preset requested and
  obtained, low-latency and exclusive per direction, rebuilds. Latency arrives with `T-002f`; the
  overlay shows "—" until it does.
- `core_ui` takes plain values, not a snapshot: it must not import `audio_engine`
  (`AGENTS.md` §5). `feature_tuner` maps the snapshot to the widget's inputs.
- The overlay is diagnostic, not dev-only: production users will be asked to read it in support.
  `PRODUCT_SPEC.md` does not place it yet. Where it is reachable from (Settings is the candidate)
  is decided here, with the owner, and recorded there.
- `TESTING.md` §3: "the regeneration path is decided with the first golden". That is this task, and
  it closes off alternatives: an ADR (superseding or extending `adr/0016`, never editing it).
- `STATE.md` known gap: `test-dart` would also run goldens locally. Tag them and exclude the tag
  from `test-dart`, so they run only in the `goldens` CI job.

## Acceptance criteria

- [ ] A `core_ui` diagnostics widget, under ~150 lines, painter or sub-widgets extracted, with doc
      comments on its public API
- [ ] Goldens: 3 widths × 2 themes, plus `textContrastGuideline` and the tap-target guidelines
      (`TESTING.md` §1 and §3)
- [ ] Goldens are tagged; `just test-dart` skips the tag; the `goldens` CI job runs it on
      `ubuntu-24.04`; `just goldens-update` regenerates by the route the ADR chooses
- [ ] An ADR records the regeneration path
- [ ] `feature_tuner` shows the overlay from the live snapshot, rebuilding without layout jank at
      ~30 Hz (the values change, not the layout); the dev text readout it replaces is removed
- [ ] Shown on the Redmi with real values, recorded in the journal
- [ ] `just verify` passes; all six CI jobs green

## Out of scope

The latency measurement itself (`T-002f`). The tuner UI proper (M3). Localisation of the labels
beyond what `core_ui` already requires.

## Implementation notes

- **Placement, decided by the owner (2026-10-22):** Settings → Diagnostics, a screen support can
  name ("open Settings, then Diagnostics"). A hidden gesture on the tuner was rejected as
  undiscoverable. `PRODUCT_SPEC.md` records it when this task builds the screen.

## Verification performed

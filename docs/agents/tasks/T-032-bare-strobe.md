---
id: T-032
title: A bare strobe on the phone, and the first golden
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

M3's largest product risk is whether a stall looks dead on a real string held by a real hand. Test it
with the bare geometry before building a design system around it, and land the first golden on the way.

## Context

Read: `T-033` (the brief), `docs/DESIGN_SYSTEM.md` §3, `docs/ARCHITECTURE.md` §5, `docs/TESTING.md` §1 and
§3, `docs/adr/0016`, `AGENTS.md` §6 ("the audio clock is the only clock"). The `flutter-ui` agent's scope.

- **The law.** `DESIGN_SYSTEM.md` contradicts itself: §1 says rate in Hz, §3 says ∝ cents. Use cents. 1 ¢
  is about 0.018 Hz on low B and 0.76 Hz on E6, so a Hz law leaves bass nearly static. Dart applies
  `Δφ = K(Δcycles − f_t · Δframes / rate)` over `T-028`'s integrals, with K ∝ 1/f_t for the cents law.
- **`StrobeClock`, pure Dart in `core_domain`,** advances an estimated frame position, slews ≤ 1 % toward
  the newest `end`, interpolates between readings and never extrapolates. It freezes with no reading,
  and re-anchors on epoch, note change or gap. A Ticker gives vsync differences only.
  *Unverified:* the Ticker's time base against `CLOCK_MONOTONIC`.
- **Known risks.** A burst of 960 frames would give 50 Hz data read at 60 Hz, and naive extrapolation
  would shimmer at 10 Hz. Read the Redmi's actual burst from the diagnostics.
  Raw wobbles about 2.22 ¢ at 20 dB SNR and the needle 0.12 ¢, yet the needle lags. `AUDIO_ENGINE.md`
  says raw; `T-028` publishes both, and this task chooses on the Redmi.
- **Cap the rate,** against wagon-wheel aliasing and to stay within WCAG 2.3.1's three flashes per second.
  **No dead-band on phase:** "in tune" comes from `dsp::lock`, not a faked stop.
- **The first golden moves here from `T-002e`,** with its regeneration ADR (`TESTING.md` §3), so M3's
  first painted widget proves the pipeline.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] `StrobeClock` has pure-Dart tests: no extrapolation, the freeze, each re-anchor, jitter with a stall.
- [ ] A painter with no widget rebuild per frame and no per-frame allocation (`DESIGN_SYSTEM.md` §3).
- [ ] One golden, tagged, with its regeneration path decided by an ADR, as `T-002e` set out.
- [ ] On the Redmi: on a real guitar a stall visibly stops, and ±1 ¢ visibly moves on low E and high E.
      Frame times recorded (60 fps). Tested with the two-band options in `T-033`, and the choice recorded.
- [ ] A new ADR, `adr/0039`, records the law, the integrand and the display direction.

## Out of scope

Tokens, fonts, states and layouts (`T-033`). Lock transition and haptics.

## Implementation notes

## Verification performed

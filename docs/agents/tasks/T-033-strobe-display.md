---
id: T-033
title: The strobe display: the owner's direction, translated into our own design
status: todo
milestone: M3
owner: unassigned
created: 2026-11-01
---

## Goal

Turn the tuner scaffold into the product screen, in the direction the owner chose on 2026-11-01: the
asymmetric sweep of Peterson's hardware (option C of three), built as Diapasón's own design. Starts
after `T-032` proves a stall looks dead. Split it into slices when started.

## Context

Read: `docs/DESIGN_SYSTEM.md`, `docs/PRODUCT_SPEC.md` §2 and §4, `T-016`. The owner's reference photos are
local only, in `strobe-tuner-examples/`. **The repo is public and they are Peterson's marketing images:
never commit them.** This table is what we take from them:

| Peterson | Diapasón |
|---|---|
| Sweep from top right, around the left, into the bottom | The same anchoring at every width; the note in the open space, lower right |
| Two concentric bands; outer segments about half the inner's length | Two bands. Prototype the outer at 2× gain (coarse and fine) against identical motion (`T-032`). Peterson's documents do not say what its bands mean, so assume nothing |
| Unlit segments stay faintly visible | An unlit-track token. Functional: a fixed frame makes a stall easier to see |
| One huge note; small accidental and octave | `NoteGlyph` the hero, accidental and octave subordinate |
| Mode top right, Concert A bottom right | The same zones, muted: preset or temperament, then A4 |
| Lit display with a soft glow; five backlight colours | Brass on warm black. One baked glow on lit segments, never a per-frame blur. Colour presets wait for M6 |
| Reflective amber LCD, dark segments (VLN StroboClip) | **The light theme**: legible outdoors, not a plain inversion |

**Where we go further than the hardware:**
- **Beginners.** These displays show no flat/sharp word and no cents, and M3's exit asks for "visible to a
  naive user". A signed cents readout and the word sit at lower right.
- **Lock** uses every channel in `PRODUCT_SPEC.md` §4: colour, stall, shape change, optional haptic.
- **Accessibility.** Live region "E, 4 cents flat"; reduced motion is a static arc whose length encodes
  the error; nothing by colour alone.

**Trade dress.** Option C is the closest of the three to Peterson's design, and this is a paid app. Keep our
own segment counts and proportions, palette and typeface. Never copy the segmented letterforms, the ghost
LCD legends, product names or the "Sweetener" wording. M7 carries a short legal review (`ROADMAP.md`). No
lawyer has assessed this.

## Acceptance criteria

- [ ] `just verify` passes; all six CI jobs green.
- [ ] `T-016` first: typed routes, before any new screen.
- [ ] `DESIGN_SYSTEM.md` rewritten to this direction, with its Hz/cents contradiction gone, the sweep
      named (`StrobeArc`, not `StrobeRing`), and how the metronome shares the geometry stated.
- [ ] Tokens and fonts in `core_ui`; font licences confirmed OFL, closing STATE's open question.
- [ ] Every state in `PRODUCT_SPEC.md` §2 renders. `too loud` needs a peak the engine does not report
      yet: add it, or record why not.
- [ ] Goldens at 3 widths × 2 themes, with contrast and tap-target guidelines. Usable at 320 dp and in
      landscape. Expanded layout stays M6.
- [ ] TalkBack announces state. Reduced motion is fully usable.
- [ ] `T-002f` and `T-002e` done; the lifecycle rows rerun.
- [ ] About five guitarists who have not used a strobe read flat, sharp and in tune with no instruction.

## Out of scope

Metronome UI (M5). Presets, temperaments and settings screens beyond what the states need.

## Implementation notes

## Verification performed

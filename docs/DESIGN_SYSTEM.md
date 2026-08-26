# Design system

This document owns the **visual and motion language**: direction, tokens, components, adaptive
behaviour, accessibility. It is a brief for implementation in `packages/core_ui`, not a description
of code that exists.

## 1. Direction

The brief is "clean, minimal, modern" — which is where most tuner apps arrive at the same dark
screen with a green needle. The way out is to take the direction from the subject rather than from
the adjective: **the instrument itself**. Machine heads, brass frets, the felt of an open case, the
lacquer on a spruce top, and — most importantly — the *strobe disc*, the display real studio tuners
have used since the 1930s.

**The signature element is a real strobe.** A ring of segments rotates at a rate equal to the
difference between the detected and target frequency. Sharp, it drifts clockwise; flat,
anticlockwise; in tune, it **stops dead**. This is not a decorative animation — it is the physics of
a strobe tuner rendered honestly, and a stalled pattern is a far more precise perceptual signal than
a needle a human eye must judge against a mark. Everything else on the screen is quiet so that this
one element carries the app.

Spend the boldness there. The rest is disciplined: generous negative space, one accent, no
gradients-for-decoration, no glassmorphism, no glow.

### Tokens (starting point — refine in T-004, then this section is the record)

| Token | Value | Use |
|---|---|---|
| `surface` | `#14100D` | Ground. Warm near-black, the inside of a case, not a blue-black |
| `surfaceRaised` | `#1E1915` | Cards, sheets |
| `ink` | `#F2EDE6` | Primary text |
| `inkMuted` | `#9A9086` | Secondary text, inactive strobe segments |
| `brass` | `#C9A227` | The instrument accent: strobe segments, active string, dial |
| `resolved` | `#5FB49C` | In tune. Desaturated jade, never a signal green |
| `drift` | `#C4705B` | Out of tune. Muted clay, never a warning red |
| `hairline` | `#2A231D` | Rules and dividers, 1 physical pixel |

Light theme is a derived inversion with the same hues at adjusted lightness; it is a first-class
theme, not an afterthought, because people tune outdoors.

**Type.** Note names are the hero and get a display face with real character — `Instrument Serif`
(OFL, and the name is not a coincidence) at large optical sizes, used *only* for the note glyph and
the tempo number. Everything else is a neutral grotesk with **tabular figures** — non-negotiable,
because a cents readout that shifts width as digits change is visually noisy at 30 updates a second.
Numeric readouts (cents, Hz, BPM) use the mono cut. Three faces, three jobs, bundled locally: no
runtime font fetching in an app that makes no network calls.

**Structure.** No decorative numbering, no eyebrow labels. The one structural device is the
**hairline arc** shared by both screens — the tuner's strobe ring and the metronome's beat ring are
the same geometry at the same radius, so switching tabs feels like turning the same object around.

## 2. Scale and spacing

4 dp base grid; spacing scale 4/8/12/16/24/32/48/64. Corner radii: 0 for full-bleed surfaces, 12 for
cards, 999 for pills. One elevation step only (`surfaceRaised` + a hairline), never a shadow stack.
Type scale: 12/14/16/20/28/40/72/120 with the two largest reserved for the note glyph and BPM.

## 3. Motion

Motion has to be earned in an app where one animation is a measuring instrument.

| Element | Behaviour | Curve / duration |
|---|---|---|
| Strobe ring | Continuous phase rotation ∝ cents error; stalls at zero | Physical, no easing |
| Needle / arc indicator | Follows the smoothed estimate | Spring, damping 0.8, stiffness 180 |
| Lock transition | Segments settle inward, colour crosses to `resolved` | 220 ms, `easeOutCubic` |
| Unlock | No animation — instant. Losing tune must not feel gentle | 0 ms |
| Beat pulse | Scale + opacity impulse arriving *on* the beat timestamp | 90 ms attack, 180 ms decay |
| Tempo dial | Direct manipulation, 1:1 with the finger, with detents at multiples of 5 | — |
| Tab change | Shared-axis transition, ring persists across the change | 300 ms, `easeInOutCubicEmphasized` |
| State change (listening→detecting) | Cross-fade only. Never a layout shift | 150 ms |

Reduced motion: the strobe becomes a static arc whose *length* encodes error; the beat pulse becomes
a discrete colour step; transitions become cross-fades. The app must remain fully usable, and the
tuner must remain fully readable, with all motion disabled.

Frame budget: 120 fps target on ProMotion. The strobe and needle are painted by a `CustomPainter`
driven by a `Listenable` inside a `RepaintBoundary` — no widget rebuilds per frame
(`ARCHITECTURE.md` §5). Nothing on either main screen may allocate per frame; hoist `Paint`, `Path`
and `TextPainter` objects into painter fields.

## 4. Components in `core_ui`

`StrobeRing` · `TuningArc` · `NoteGlyph` · `CentsReadout` · `StringSelector` · `BeatRing` ·
`TempoDial` · `TapTempoTarget` · `AccentPatternEditor` · `SegmentedControl` · `SettingRow` ·
`PermissionGate` · `AppScaffold`.

Each ships with a golden test at three widths and both themes, and a widget-book entry. None of them
imports Riverpod, `dart:io`, or a plugin — they take values and callbacks. That constraint is what
makes them testable and reusable, and it is enforced by the import lint.

## 5. Iconography and assets

A small custom set drawn as `CustomPainter`s or bundled SVGs at 24 dp — no icon-font dependency for
twelve icons. App icon: the fretboard-and-fork double meaning, legible at 48 px. Adaptive icon on
Android with a real monochrome layer for themed icons; the full asset matrix is generated from a
single source by `just icons`.

## 6. Adaptive layout

Breakpoints follow the Material 3 window size classes, applied to the *window*, never the device:

| Class | Width | Layout |
|---|---|---|
| Compact | < 600 dp | Bottom `NavigationBar`; ring fills the width; controls in a bottom sheet |
| Medium | 600–839 dp | `NavigationRail`; ring centred with controls beside it |
| Expanded | ≥ 840 dp | Rail + two-pane: ring on the left, strings/beat editor on the right |

Landscape phones use the Medium layout regardless of width class — a vertical ring in a 400 dp-tall
window is the layout most tuner apps get wrong. Tablet layouts are designed, not stretched: the
extra space shows all strings and the accent editor simultaneously rather than enlarging the ring.
Foldables: handle hinge insets via `MediaQuery.displayFeatures`; do not place the ring under a hinge.

## 7. Accessibility

Every requirement in `PRODUCT_SPEC.md` §4 is a design constraint here. Specifically:
semantic labels on every painted element (a `CustomPainter` is invisible to a screen reader unless
you build a `Semantics` tree — this is the most commonly missed item in a painted UI); a live region
announcing "E, 4 cents flat" at a throttled rate; contrast checked in the golden test suite; text
that reflows rather than truncates at the largest dynamic type sizes; and focus order that follows
visual order.

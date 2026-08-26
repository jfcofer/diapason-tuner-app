# 0004 — Riverpod 3 for discrete state, Listenable for continuous

**Status:** Accepted · 2026-08-25

## Context

The UI has two very different kinds of state. Discrete: which instrument, is the metronome playing,
which note was detected, is it locked. Continuous: cents deviation and strobe phase at 30–50 Hz,
beat animation phase at 120 fps. Treating them the same is how painted, high-frequency Flutter UIs
end up janky — a `setState` or provider rebuild per frame walks the widget tree for values only a
`CustomPainter` consumes.

## Decision

Riverpod 3 with code generation for discrete state. Continuous values bypass the widget tree
entirely: the engine snapshot stream feeds a `ValueNotifier`/`Listenable` handed directly to a
`CustomPainter` inside a `RepaintBoundary`.

## Consequences

**Good.** One repaint of one boundary per frame instead of a subtree rebuild. Discrete state keeps
Riverpod's testability, lifecycle handling and override-based fakes. The split is easy to state and
therefore easy to review: *if it changes every frame, it does not go through a provider.*

**Bad.** Two mechanisms to learn, and a reviewer must notice when a value has migrated from one
category to the other. Code generation adds `build_runner` to the loop.

**Rejected: BLoC.** The ceremony is not repaid at this size, and its event/state stream model is a
poor fit for latest-wins snapshots.

**Rejected: everything through `setState`.** Unusable at 120 fps in a painted UI, and untestable.

**Rejected: signals-style libraries.** Attractive for exactly this problem, but a smaller ecosystem
and less battle-tested tooling than Riverpod for the discrete half of the app. The `Listenable`
split gets most of the benefit with none of the bet.

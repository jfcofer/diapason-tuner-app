# 0036 — The tuner listens by itself after one consent

**Status:** Accepted · 2026-11-01 (`T-024`; the owner, 2026-11-01)

## Context

`PRODUCT_SPEC.md` §1 and §2 say the app opens already listening: zero friction is the second
differentiator. `T-002d` built the opposite for M1. Listening takes a tap, and returning to the
tuner does not restart it (`PLATFORM_AUDIO.md` §5, L11), for privacy. `STATE.md` left the conflict
for the owner to settle before M3.

## Decision

- **First run:** the tuner explains why it needs the microphone and that no audio leaves the device
  or is stored, then the user asks for it. The system prompt follows that tap, and never appears
  unprompted at launch.
- **After that, a visible tuner listens by itself:** on launch and on returning to the screen or the
  app.
- **It always stops when it is not visible:** in the background, on the lock screen, or when another
  screen covers it. `engineTarget`'s rule ("nothing listens unless the tuner is in front of the
  user") does not change; only what `listenWanted` starts as does.
- **Listening is visible and can be paused.** The tuner shows that it is listening and has a pause
  control. A pause holds for the rest of that run of the app, across backgrounding and returning. A
  new launch listens again.
- **Denied stays denied.** The app never asks again by itself: it shows the recovery UI, which leads
  to the system's settings (L7).

## Consequences

**Good.** The spec's core loop holds from the second launch on. The privacy posture stays
inspectable: no network permission (`adr/0010`), and audio never leaves the ring buffer.

**Bad.** The microphone opens without a tap, so the system's indicator lights on every return to the
tuner, and the first launch still costs a tap. Row L11 changes meaning and its patrol proof no longer
applies, so it is `open` until `T-031` changes the code and reruns it on the Redmi.

## Alternatives rejected

- **Always tap to listen.** The strongest privacy story, and simple. It adds a tap to every tuning
  and gives up a stated differentiator.
- **The same behaviour behind a setting.** More states to test on the lifecycle matrix, for a choice
  the pause control already gives.
- **Ask for the microphone on first launch, unexplained.** Denial rates rise, and a denied permission
  is expensive to recover on Android.

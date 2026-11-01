# 0035 — Android only for 1.0

**Status:** Accepted · 2026-11-01 (`T-024`; the owner, 2026-11-01)

## Context

The product was specified for Android and iOS, with an exit that named four reference devices. The
owner has one physical device, a Redmi 23117RA68G, and no Mac and no iPhone. So:

- M1's exit could never be met. `T-002` had already amended "all four" to "every device available"
  in its task file, while `ROADMAP.md` kept the old wording.
- A tuner's accuracy rests on what the microphone delivers after the OS has processed it. A
  Simulator cannot show that, so an iOS release could not be verified at all.
- iOS was drawing work with nothing to show for it: the CoreAudio backend (`T-002c`), SwiftPM for
  `audio_engine` (`T-017`), the Xcode pin (`T-004`), and App Store and privacy-manifest rules.

## Decision

**Version 1.0 ships on Android only, through Google Play.** iOS moves to `ROADMAP.md`'s post-1.0 list.

- **The one reference device is the Redmi.** Exits that named iOS, ProMotion, "both platforms" or
  "both stores" now name Android, the display's own refresh rate, and Google Play.
- **`T-002c` is abandoned, and `T-017` is blocked on a decision to do iOS.** `T-002c` could not stay
  blocked, or its parent `T-002` could never close. A later iOS effort starts from its file.
- **The seams stay.** The `AudioBackend` trait, `apps/diapason/ios`, `tools/ios/` and the iOS parts of
  `PLATFORM_AUDIO.md` are kept and marked post-1.0. No code is deleted.
- **`build-ios` stays a required check for as long as it stays green at no cost.** The first time
  keeping it green would take iOS-only work, such as a Flutter bump turning the SwiftPM warning into
  an error, the job is removed rather than the work done. Removing it means changing the ruleset,
  which only the owner can do.

To reverse this: a new ADR, once an iPhone and a Mac (or a hosted Mac with a device) are available.

## Consequences

**Good.** M1 can close. Work on iOS stops until it can be verified. The App Store's Xcode 26 upload
rule and the CocoaPods trunk freeze of 2 December 2026 stop being near-term concerns.

**Bad.**
- One budget phone is all the evidence. An input preset, a high-pass or a latency path that differs
  on another OEM stays invisible until M7's closed test. A second Android OEM is an open question in
  `STATE.md`, due before M7.
- A market is deferred, and a one-time purchase on one store earns less.
- `build-ios` spends macOS CI time on a target that will not ship in 1.0, until it is removed.

## Alternatives rejected

- **Keep both platforms in step, buying a used iPhone and Mac time.** It retires iOS risk earlier,
  and costs money and setup before the product shows a note on any phone.
- **Keep iOS "CI-only" in 1.0.** That ships a tuner whose microphone path nobody has heard.
- **Delete the iOS project and job now.** It saves CI minutes today, and throws away working
  generated-project tooling that costs nothing while it stays green.

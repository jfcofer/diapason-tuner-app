# 0010 — No telemetry, no network calls

**Status:** Accepted · 2026-08-25

## Context

Analytics are the default in mobile development, and there are real arguments for them: knowing
which features are used, catching crashes, measuring retention. There are also costs: a network
permission, privacy-manifest and data-safety declarations, an SDK in the process, a category of
bug that only reproduces in the field, and a promise to users that becomes harder to keep with every
release.

## Decision

The app makes **zero network requests**. No analytics, no ads, no remote config, no crash reporting
enabled by default. Opt-in crash reporting may be added later, off by default, sending crashes only.

## Consequences

**Good.** The store listing can say something true and rare, which is a genuine differentiator in
this category. Data-safety and privacy-manifest answers are trivially "none". No SDK in the audio
process. Nothing to breach. Works on a plane, in a rehearsal room, in a basement — which is where
this app is used.

**Bad.** We are blind to real-world usage and to crashes on devices we do not own. Mitigations: the
diagnostics overlay (`CI_RELEASE.md` §6) makes a single support email diagnostic; the staging cohort
runs opt-in crash reporting; the manual device matrix in `TESTING.md` §7 is not optional.

**This is architectural, not a preference.** Reversing it means a network permission, new store
declarations, and a broken promise. Any task that appears to require a network call stops and
escalates instead.

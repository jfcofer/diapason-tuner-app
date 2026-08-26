# Roadmap

Milestones are ordered by **risk retired per unit of work**, not by what is most visible. The
audio spine is proven end to end on both platforms before a single pixel of the real UI is drawn,
because that is where this project can actually fail.

Each milestone lists exit criteria. A milestone is not complete until every one is objectively true.
`docs/agents/STATE.md` names the current milestone; tasks live in `docs/agents/tasks/`.

---

## M0 — Foundations
Repo skeleton, workspaces, toolchain pins, `just` recipes, CI green on an empty app, agent context
system in place.

**Exit:** `just setup && just verify` passes from a clean clone on macOS and Linux · CI green ·
dependency-direction check enforced · dev/stg/prod flavours install side by side · `just rename`
works.

## M1 — Audio spine
The riskiest 200 lines in the project. One duplex stream on both platforms, through the real
backends, with a trivial payload: input RMS out, a sine in.

**Exit:** mic permission flow works on both OSes · measured round-trip latency reported in the
diagnostics overlay on all four reference devices · Android reports which input preset it obtained ·
stream survives every row of the lifecycle matrix · zero allocations in the callback, proven by
test · native lib 16 KB-aligned.

## M2 — Pitch core
`dsp` crate, offline, no UI. This is where accuracy is won.

**Exit:** every accuracy budget in `AUDIO_ENGINE.md` §1 met against the fixture set · octave-guard
tests pass on pinch harmonics and low-B fixtures · criterion baselines committed · fixture generator
reproducible · `core_domain` note/temperament math agrees with `dsp` on the shared table.

## M3 — Tuner MVP
The strobe ring, the note glyph, the cents readout, chromatic mode. Real detection driving real UI.

**Exit:** end-to-end latency budget met on device · strobe stalls at pitch and the stall is visible
to a naive user without instruction · every tuner state from `PRODUCT_SPEC.md` §2 renders · goldens
committed · 120 fps sustained on ProMotion, 60 fps on the budget Android · usable at 320 dp.

## M4 — Metronome core
Sample-accurate scheduler, procedural clicks, signatures, subdivisions, accents. Minimal UI.

**Exit:** the 30-minute zero-drift scheduling test passes at 47 tempos · tempo and signature changes
preserve phase · background playback works on both platforms with lock-screen controls · voice pool
never allocates.

## M5 — Metronome UI and sync
Beat ring, tempo dial, tap tempo, accent editor, haptics, visual sync via audio timestamps.

**Exit:** the pulse lands on the beat, verified by high-frame-rate video against the audible click ·
haptics locked to the audio clock, never to a Ticker · latency calibration flow works over
Bluetooth · trainer and gap trainer complete.

## M6 — Production polish
Adaptive layouts, accessibility, localisation, theming, settings, onboarding, error recovery.

**Exit:** every accessibility requirement in `PRODUCT_SPEC.md` §4 verified with a screen reader on
both platforms · en + es complete including note-name systems · expanded layout designed, not
stretched · reduced-motion path fully usable · every error state has a specific recovery
affordance · cold start under 1.2 s on the budget Android.

## M7 — Release
Signing, store assets, privacy manifests, release pipeline, beta, submission.

**Exit:** `just release-check` clean · both stores accept the build · staging cohort crash-free
above 99.5 % · rollback procedure written and rehearsed once.

---

## Post-1.0 (do not let these influence 1.0 design beyond existing seams)

Sample-based click packs · setlists with per-song tempo · polyphonic chord tuning · watch companion ·
audio-interface input · MIDI clock out · practice statistics stored locally · desktop build (the
architecture already permits it via the cpal backend, which is why that backend exists).

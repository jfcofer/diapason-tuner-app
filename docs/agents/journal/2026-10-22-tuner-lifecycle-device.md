# 2026-10-22 — tuner lifecycle on the Redmi

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002d (part 1)  **Milestone:** M1

## Done
- **Measured both `dumpsys` formats on the Redmi before parsing anything**, then wrote
  `just mic-clients`.
  - **Open clients** come from the `media.audio_policy` Inputs section.
  - **Timestamped events** come from `dumpsys audio`'s `rec start|update|stop|release … silenced
    pack:` log.
- **Patrol 5/5:** the 2 contract tests, plus L1/L2 by Home, L1/L2 by recents, and L11.
- **By hand, timed against the activity's own `wm_on_stop_called`:**

  | Row | Event | Released or back | Measured from |
  |---|---|---|---|
  | L1 | Home | 50 ms | `onStop` |
  | L1 | Recents | 38 ms | `onStop` |
  | L8 | Lock | 142 ms | the key press |
  | L2 | Return | microphone back in 0.42 s | the resume |
- **L7:** `pm revoke` killed the process; the relaunch showed the recovery UI.
- **L5:** a Bluetooth A2DP device connected and removed mid-session, and the stream rebuilt both
  times. L6's rebuild works too, but it also asks to recalibrate latency, so it went to `T-002f`.

## Tried and abandoned
- **Installing `app-dev-debug.apk` after a patrol run for manual checks.** Patrol builds that APK
  with the test bundle as its entry point. Rebuild with `just run android` instead.

## Surprises
- **patrol_cli sends Google Analytics by default.** Its consent file on this host had said
  `enabled: true` since the first T-013 run. It also fails offline, because it asks pub.dev for
  updates first: DNS for pub.dev was down today. `tools/patrol.sh` now sets
  `PATROL_ANALYTICS_ENABLED=false` (CI detection alone does not stop the send path) and `CI` (to
  skip the check).
- **HyperOS stops the activity behind recents**, so the app is `hidden` there and the recents test
  holds. Android takes about 1 s from pause to stop on that path.
- **A "permission dialog" in a UI dump was a stale `/sdcard/ui.xml`.** `uiautomator dump` cannot
  idle while the tuner redraws.
- **This ROM reports silencing.** An earlier install shows `rec update … silenced` before
  `release`. That is good news for part 2 (L10).

## Left for next session
- Push the branch and open the part 1 PR.
- L3/L4 need a call to the Redmi.
- Part 2: L10, starting from the `silenced` events above.

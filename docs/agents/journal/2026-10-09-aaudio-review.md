# 2026-10-09 — aaudio-review

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b (part 1 of 3)  **Milestone:** M1

## Done
- Fixed what the adversarial review of the AAudio backend found, before merge:
  - **A lost microphone was silent:** a failed read only padded with silence. It now sets
    `disconnected`, and the input stream has an error callback too.
  - **`host_time_ns` was render time,** against `AUDIO_ENGINE.md` §6. It is now presentation time,
    extrapolated from `getTimestamp` on the callback thread.
  - **The worst-callback figure included the first callback's drain.** Without it: release
    281–571 µs, debug 581–653 µs, against the 20 ms period.
  - **A failed output close freed state a callback might still use.** It now leaks it.
  - **The device script** uses cargo-ndk's runner instead of guessing binaries by mtime, and
    checks the canary for the allocator's message.
  - **The docs claimed too much.** The trap sees only Rust's allocator, `read` and
    `getTimestamp` can take a platform mutex on the legacy path, and 32-bit `clock_gettime` may
    enter the kernel. ADR 0020, `AGENTS.md` §6 and `AUDIO_ENGINE.md` §7 now say so.

## Tried and abandoned
- **A custom adb runner script.** cargo-ndk 4 already installs `cargo-ndk-runner`, which takes
  precedence and does the same thing. Deleted.

## Surprises
- **I put an allocation on the RT path while fixing the review:** a `StreamHandle::new()` in
  `drain`. I caught it before running, by reading the diff with the RT rules in mind. The trap would
  have aborted the device run. `read_input` is now pure, and its callers do the counting.
- **The evidence in the first ADR draft came from binaries older than the committed source**
  (the reviewer checked mtimes). Every figure is now from runs at the committed code.

## Left for next session
- Part 2. Its new debts are in the task: the release budget measured from the app, backlog
  shedding, buffer growth on xruns, and `PermissionDenied` mapping.

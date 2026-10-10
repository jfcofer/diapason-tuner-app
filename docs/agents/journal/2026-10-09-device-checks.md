# 2026-10-09 — device-checks

**Agent:** Claude Code (Opus 5.5)  **Task:** T-002b part 2a  **Milestone:** M1

## Done
- **The owner ran `just test-integration-android` on the Redmi.** Both halves passed with the debug
  allocation trap armed. The refused microphone fails to open with `-896` (`AAUDIO_ERROR_INTERNAL`).
- **The owner ran `just run android` and did the hand checks.** I read the kept logcats:
  - **Fast path:** the app gets `AUDIO_OUTPUT_FLAG_FAST` and `AUDIO_INPUT_FLAG_FAST`, but not
    MMAP, because the app's policy is "never". The STATE open question is closed: only the shell
    user is refused.
  - **Headphone replug:** each wired plug or unplug made AAudio mark the stream disconnected. The
    session closed both streams and reopened them on the new device. That happened several times,
    with no crash.
  - **Revocation:** revoking the microphone in Settings killed the running app ("Lost connection
    to device"), as `PlatformMicrophonePermission`'s doc says.
- **Fixed `just run android`.** `flutter run -d` matches a device, not a platform. The recipe
  passes adb's serial now.

## Tried and abandoned
- **Mapping `-896` to `PermissionDenied`.** It is AAudio's generic internal error, so 2b must use
  the permission status instead.

## Surprises
- **In the background the platform mutes the microphone and reports no error.** The log shows
  13 s of data, then 17 s of `[mute]`, starting when the app lost the foreground. The tuner would
  hear exact zeros without knowing why. Part 3's lifecycle has to release the microphone in the
  background (`PLATFORM_AUDIO.md` §5), as the matrix already says.
- **The `AudioRecord` under AAudio logs `inputSource 0`** although `VOICE_RECOGNITION` was
  requested. It may only be how the legacy path logs it. 2b checks whether the preset is applied.

## Left for next session
- PR for part 2a (owner approves the push), then 2b.

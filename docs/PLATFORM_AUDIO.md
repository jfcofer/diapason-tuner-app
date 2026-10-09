# Platform audio and OS integration

This document owns everything **outside** the algorithm: how the stream is opened on each OS, what
the store requires in 2026, and the platform behaviours that break naive audio apps. Algorithms are
in `AUDIO_ENGINE.md`.

## 1. The backend trait

`rust/crates/audio_io` exposes one trait and four implementations. The engine knows nothing else.

```rust
pub trait AudioBackend {
    fn name(&self) -> &'static str;                  // for the diagnostics overlay
    fn open(&mut self, cfg: StreamConfig, cb: Box<dyn AudioCallback>) -> Result<StreamHandle>;
    fn actual_config(&self) -> Option<StreamConfig>; // device may not honour the request
    fn timestamp(&self) -> Option<StreamTimestamp>;  // frame index ↔ host clock
    fn close(&mut self) -> Result<()>;               // idempotent; drops the callback
}

pub trait AudioCallback: Send + 'static {            // runs on the RT thread: AGENTS.md §6
    fn process(&mut self, input: &[f32], output: &mut [f32], info: &CallbackInfo);
}
```

Buffers are interleaved `f32`, and block sizes vary up to `max_block_frames`. `StreamHandle` is a
lock-free view of the stream's counters (callbacks, frames, largest block) that never blocks either
side. Every backend must pass the conformance suite in `audio_io/src/conformance.rs`. The rustdoc
on these types is the full contract.

| Impl | Platform | Notes |
|---|---|---|
| `OboeBackend` | Android | AAudio via Oboe; the only production Android path |
| `CoreAudioBackend` | iOS/iPadOS | AURemoteIO Audio Unit; the only production iOS path |
| `CpalBackend` | macOS/Windows/Linux | Desktop dev loop only — never shipped |
| `OfflineBackend` | any | Deterministic, faster-than-real-time; all tests |

Duplex (simultaneous in and out) is required: the tuner may run while the metronome plays. Open one
duplex stream rather than two, so both share a clock.

## 2. Android

**Stream configuration** — Oboe with `PerformanceMode::LowLatency`, `SharingMode::Exclusive` with
automatic fallback to `Shared`, `Usage::Media`, float samples, mono in / stereo out.

Query and *use* the device's native values rather than forcing 48 kHz/256:
`AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE` and `PROPERTY_OUTPUT_FRAMES_PER_BUFFER`. Requesting a
buffer the device does not want is the most common cause of the fast path silently not being
granted. Set buffer size to a small multiple of the burst size and let Oboe's automatic latency
tuning do the rest.

**Input preset matters more than anything else for tuner accuracy.** Android's default input applies
AGC, noise suppression and a voice-band filter, all of which destroy pitch content. Request
`InputPreset::Unprocessed` when `PackageManager` reports
`FEATURE_AUDIO_LOW_LATENCY` + the device advertises
`AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED`; otherwise fall back to
`InputPreset::VoiceRecognition`, which at least disables AGC on most devices. Never
`VoiceCommunication`. Record the chosen preset in the snapshot so a support report can tell us
which path a device took.

**Permissions** — `RECORD_AUDIO` at runtime, requested only when the user first opens the tuner,
with a pre-permission explanation screen. The metronome must work fully with the permission denied.

**Foreground service** — required for the metronome to keep playing when the app is backgrounded:
`android:foregroundServiceType="mediaPlayback"`, a `MediaSession` with transport controls, and
`POST_NOTIFICATIONS` on API 33+. The tuner does **not** run in the background; the mic is released
the moment the app is not foreground, and the UI says so.

**2026 platform requirements (verify before every release):**
- `targetSdk = 36` (Android 16). Google Play requires API 36 for new apps and updates from
  **31 August 2026**; extensions run to 1 November 2026. `minSdk = 26`.
- **16 KB memory page support is mandatory** for apps with native libraries on recent devices. We
  pin **NDK r28+** (`tools/versions.env`), which aligns `arm64-v8a` and `x86_64` to 16 KB *by
  default* — the old `-Wl,-z,max-page-size=16384` flag is only load-bearing on r27 and below.
  Verified empirically under both FRB backends during `T-001a` (`LOAD align 0x4000`). Still inspect
  the shipped `.so` in CI rather than trusting the NDK version: a silent regression here is a crash
  on affected devices, not a warning.
- Edge-to-edge is enforced; handle insets explicitly.
- Predictive back must be supported and tested.
- 16 KB, edge-to-edge and target API checks live in `just check-android-release`.

**Device disconnect** — Oboe reports `ErrorDisconnected` from the callback. Do not rebuild the
stream on the audio thread. Signal a normal-priority thread, close, reopen with backoff, and surface
a `route_changed` flag in the snapshot so the UI can show a brief, non-alarming indicator.

## 3. iOS / iPadOS

**AVAudioSession** is configured from Swift (the Rust side does not own the session):

- Category `.playAndRecord`, options `[.defaultToSpeaker, .allowBluetoothA2DP, .mixWithOthers]`.
  `.mixWithOthers` is deliberate: a player practising along with a backing track should not have it
  killed by opening a tuner.
- **Mode `.measurement` while the tuner is active.** This bypasses the system's input processing
  (AGC, EQ) the same way `Unprocessed` does on Android, and is the single biggest accuracy lever on
  iOS. Switch back to `.default` when only the metronome runs, because `.measurement` also reduces
  output gain noticeably.
- `setPreferredSampleRate(48000)`, `setPreferredIOBufferDuration(0.005)`. Read back what you got.
- `UIBackgroundModes: [audio]` in `Info.plist` for background metronome.

**Interruptions and route changes** — subscribe to `AVAudioSession.interruptionNotification` and
`routeChangeNotification`. On `.began`, stop cleanly and remember transport state. On `.ended` with
`.shouldResume`, restart. On route change of reason `.oldDeviceUnavailable` (headphones unplugged),
pause the metronome, matching platform convention. Also handle `mediaServicesWereResetNotification`
by tearing down and rebuilding the entire engine — rare, but it is the one case where holding stale
handles hard-crashes.

**Info.plist / privacy**
- `NSMicrophoneUsageDescription`, written for a human: what it is used for and that audio never
  leaves the device. Localised (en + es).
- **`PrivacyInfo.xcprivacy` is required.** Declare `NSPrivacyTracking = false`, no tracking domains,
  no collected data types, and required-reason API declarations for anything used
  (`UserDefaults` → `CA92.1`, file timestamps → `C617.1` if applicable). Third-party SDKs must ship
  their own manifests; this is a reason to have almost none.
- App Store data-safety answers: *no data collected*. Keep it that way; it is a differentiator worth
  more than any analytics dashboard.

**Deployment target** iOS 15.0. Tablet-class layouts on iPad; support multitasking, Stage Manager
sizes, and rotation.

## 4. Latency compensation

Both platforms report round-trip latency. Store it, expose it in Settings as a calibration slider
(default: measured value), and use it for the visual/haptic sync in `AUDIO_ENGINE.md` §6. Offer a
manual calibration flow (tap along to the click; the app computes your offset) — musicians with
Bluetooth headphones will need it, and Bluetooth latency is not reliably reported.

**Bluetooth** — warn once when the output route is Bluetooth while the metronome is active. A2DP
latency of 150–250 ms makes a metronome unusable for ensemble timing and users blame the app.

## 5. Lifecycle matrix

Each row is an integration test in `test/integration/lifecycle_test.dart`:

| Event | Tuner | Metronome |
|---|---|---|
| App backgrounded | Stop, release mic | Continue (FGS / background audio) |
| App foregrounded | Restart if it was active | Continue |
| Phone call begins | Stop | Pause, remember |
| Call ends | Resume if it was active | Resume if it was playing |
| Headphones unplugged | Continue (route change) | Pause |
| Bluetooth connects mid-session | Rebuild stream, recalibrate latency | Rebuild, warn once |
| Permission revoked in Settings | Stop, show recovery UI | Unaffected |
| Screen locked | Stop | Continue, show lock-screen controls |
| Low-power mode | Reduce update rate to 20 Hz | Unaffected |
| Another app takes exclusive audio | Stop with a clear message | Pause |

## 6. Battery and thermals

The tuner is the expensive mode. Reduce the snapshot rate from 30 Hz to 20 Hz under low-power mode,
stop analysis entirely (keeping the stream open) after 60 s of silence below the gate, and drop the
strobe animation to 30 fps when the device reports thermal pressure. Keep-awake is on only while the
tuner or metronome is active, and is released on every exit path — including the crash path.

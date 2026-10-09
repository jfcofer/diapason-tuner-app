# Architecture

This document owns the **shape of the system**: which components exist, how they depend on each
other, how data and threads move between them. Algorithms live in `AUDIO_ENGINE.md`; platform
specifics in `PLATFORM_AUDIO.md`; directory contents in `REPO_LAYOUT.md`.

## 1. The one decision everything follows from

**Rust owns audio end to end. Dart never sees a sample.**

A tuner and a metronome are both real-time problems wearing a UI. Pitch detection needs a
guaranteed-latency path from microphone to result; a metronome needs beats placed on exact sample
indices. Dart's garbage collector, its event loop, and the platform channel hop are all sources of
unbounded latency, so none of them may sit inside those paths.

The consequence: the audio callback runs on an OS real-time thread inside Rust, from device input
to device output, and communicates with the rest of the app only through lock-free structures. The
UI is a *subscriber* to a state snapshot, never a participant in timing.

## 2. Component graph

```
┌──────────────────────────────────────────────────────────────────────┐
│ apps/diapason            app shell: bootstrap, flavors, router, DI   │
└───────────────┬──────────────────────────────────────────────────────┘
                │
┌───────────────▼──────────────┬───────────────────┬───────────────────┐
│ feature_tuner                │ feature_metronome │ feature_settings  │
│ presentation + view models   │                   │                   │
└───────────────┬──────────────┴─────────┬─────────┴─────────┬─────────┘
                │                        │                   │
┌───────────────▼────────┬───────────────▼────────┬──────────▼────────┐
│ core_ui                │ core_domain            │ core_platform      │
│ tokens, primitives,    │ pure Dart model:       │ permissions,       │
│ painters, motion       │ notes, temperaments,   │ haptics, wakelock, │
│                        │ tunings, cents math    │ lifecycle, prefs   │
└────────────────────────┴───────────┬────────────┴────────────────────┘
                                     │
┌────────────────────────────────────▼─────────────────────────────────┐
│ packages/audio_engine    Dart facade + generated FRB bindings         │
│                          exposes: EngineCommand in, EngineSnapshot out│
└────────────────────────────────────┬─────────────────────────────────┘
                                     │  FFI (flutter_rust_bridge v2)
┌────────────────────────────────────▼─────────────────────────────────┐
│ diapason_ffi             API surface only — no logic                  │
├───────────────────────────────────────────────────────────────────────┤
│ rust/crates/session      stream supervisor: opens, rebuilds with       │
│                          backoff, replays desired state (adr/0022)     │
├───────────────────────────────────────────────────────────────────────┤
│ rust/crates/engine       RT graph, command queue, snapshot publisher,  │
│                          metronome scheduler, tuner pipeline, state    │
├──────────────────────────────┬────────────────────────────────────────┤
│ rust/crates/dsp              │ rust/crates/audio_io                    │
│ pure computation:            │ backends behind one trait:              │
│ NSDF/MPM, filters, resample, │ AAudio (Android) · AudioUnit (iOS) ·    │
│ click synthesis, smoothing   │ cpal (desktop dev) · Offline (tests)    │
│ no I/O · no alloc in hot path│                                         │
└──────────────────────────────┴────────────────────────────────────────┘
```

**Dependency rule:** arrows point downward only, and are enforced in CI (`just check-deps` parses
`pubspec.yaml` and `Cargo.toml` files and fails on an upward or sideways edge). `feature_*`
packages never import each other. `dsp` depends on nothing but `core` numeric crates.

## 3. Why these seams

| Seam | Bought with it |
|---|---|
| `dsp` has no I/O | The hardest code in the project runs in milliseconds on a laptop against fixture buffers. Accuracy is a unit test, not a field report. |
| `audio_io` is a trait | The same engine runs on a phone, on CI with no audio device, and in a benchmark harness. Platform bugs stay in one crate. |
| `ffi` has no logic | The generated bridge is a mechanical transformation. Nothing worth testing lives where it is awkward to test. |
| `core_domain` is pure Dart | Note names, temperaments and cents math are shared by UI and settings, and are trivially testable. They are duplicated in Rust *by design* (see below). |
| `feature_*` are packages, not folders | The compiler enforces the boundary. A folder does not. |

**On the deliberate duplication:** cents/note math exists in both `dsp` (for detection) and
`core_domain` (for display and settings). This is intentional — a shared FFI type for it would
couple the UI to the engine's release cycle for no benefit. The two are kept honest by a shared
fixture file (`fixtures/note_table.json`) that both test suites assert against. Documented in
`adr/0007-domain-model-duplication.md`.

## 4. Threading and data flow

Four threads matter:

1. **Audio RT thread** (owned by the platform, entered by `audio_io`) — runs the callback. Reads
   commands from a lock-free SPSC queue, writes results into a triple-buffered snapshot slot and
   input audio into a ring buffer. Never blocks. Rules: `AGENTS.md` §6.
2. **Analysis thread** (Rust, normal priority) — for work too heavy for the callback: FFT-based
   NSDF over the largest windows. Consumes the ring buffer, produces pitch estimates. Deliberately
   *not* in the callback so that a slow frame drops an analysis update rather than glitching audio.
3. **Dart UI isolate** — subscribes to the snapshot stream, renders. Sends commands.
4. **Dart platform/lifecycle** — permissions, session activation, background transitions.

```
mic ──▶ [RT callback] ──▶ ring buffer ──▶ [analysis thread] ──▶ triple-buffered
          │  metronome                                            EngineSnapshot
          │  render                                                     │
          ▼                                                    ~30 Hz poll/stream
       speaker                                                          ▼
                                                              Riverpod ──▶ widgets
   Dart commands ──▶ SPSC command queue ──▶ consumed at top of RT callback
```

**Snapshot, not events.** The engine publishes an immutable `EngineSnapshot` (current frequency,
cents, note, clarity, lock state, input level, beat index, beat phase, transport state). The UI
reads the latest and never queues up. A dropped snapshot is invisible; a queued one is jank. Beat
*onsets* additionally carry the sample index and a converted host timestamp so the UI can animate
ahead of the click rather than reacting to it (`AUDIO_ENGINE.md` §6).

**Backpressure:** the stream is throttled in Rust, not Dart. If the UI thread stalls, the engine
notices the unread slot and simply overwrites it.

## 5. Flutter-side architecture

**MVVM with Riverpod 3, code-generated.** Chosen over BLoC for the volume of ceremony a
two-screen app should not pay, and over raw `ChangeNotifier` for testability and lifecycle
handling. `adr/0004-state-management.md`.

- `EngineSnapshot` arrives as a `Stream`; a `NotifierProvider` per feature maps it to a view model.
- **High-frequency values bypass the widget tree.** The needle, the strobe phase, and the beat
  pulse are driven by a `ValueListenable`/`Listenable` handed straight to a `CustomPainter`, so a
  50 Hz update repaints one `RepaintBoundary` instead of rebuilding a subtree. Riverpod carries
  *discrete* state (which note, locked or not, transport, settings), not the continuous signal.
  This distinction is the single biggest performance decision on the Flutter side.
- Settings persist through `core_platform`'s `SettingsStore` (SharedPreferencesAsync), are read
  once at boot, and are pushed into the engine as commands. The engine is never the source of truth
  for user preferences; it is told.
- Navigation: `go_router` with typed routes. Two primary destinations plus settings — a
  `NavigationBar` on compact widths, `NavigationRail` from medium up (`DESIGN_SYSTEM.md` §6).

## 6. Lifecycle

The engine is a state machine, and the Dart side mirrors it rather than inventing its own:

```
Uninitialised ──init()──▶ Idle ──startTuner()──▶ Tuning ──┐
                           ▲ ▲                            │
                           │ └──stopAll()─────────────────┘
                           └────startMetronome()──▶ Playing
                                    (Tuning + Playing may overlap: duplex stream)
```

Transitions that matter and are easy to get wrong, each covered by an integration test:
app backgrounded, phone call interrupts, headphones unplugged mid-beat, permission revoked from
Settings while running, device audio route changes, screen locked with metronome playing, low-power
mode. Behaviour for each: `PLATFORM_AUDIO.md` §5.

## 7. Error handling

- Rust returns `Result` across FFI, and a panic is never how an error is reported. Release builds
  abort on panic, so the FFI surface denies `unwrap`, `expect`, `panic!` and unchecked indexing at
  compile time (`adr/0021`).
- Errors are typed and *actionable at the UI*: `PermissionDenied`, `DeviceUnavailable`,
  `SampleRateUnsupported`, `Interrupted`. The UI maps each to a specific recovery affordance —
  never a generic snackbar.
- The engine self-heals on stream disconnect (device change, route change) by rebuilding the
  stream on a non-RT thread with exponential backoff, and reports the transition in the snapshot.

## 8. What is deliberately not here

No dependency injection framework beyond Riverpod. No repository layer — there is no remote data.
No BLoC. No code generation beyond FRB, Riverpod, and l10n. No abstraction over `dart:ui` painting.
No plugin for audio. Each of these was considered and rejected; the reasoning is in `docs/adr/`.

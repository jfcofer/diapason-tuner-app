# 0022 — A `session` crate supervises the stream, and replays desired state after every rebuild

**Status:** Accepted · 2026-10-09

## Context

`T-002b` part 2a puts a real stream behind the FFI. Someone has to own that stream off the audio
thread: open it, notice it break, rebuild it with backoff (`PLATFORM_AUDIO.md` §2,
`ARCHITECTURE.md` §7), and publish ~30 Hz snapshots for Dart. That owner needs a thread, a channel
or lock, and a clock. None of the existing crates can host it:

- **`engine`:** its `clippy.toml` denies `Mutex::lock`, `Instant::now` and friends crate-wide,
  and `AUDIO_ENGINE.md` §7 allows it no cross-thread primitive beyond `rtrb` and `triple_buffer`.
  Those rules keep the RT half honest, and a supervisor inside would need them relaxed.
- **`diapason_ffi`:** contains no logic (`AGENTS.md` §5).
- **`audio_io`:** platform code, beneath `engine`. A supervisor there would have to depend on the
  engine above it.

There is a second problem. A backend drops its callback when the stream closes (`PLATFORM_AUDIO.md`
§1). On Android it also drops it when the input fails to open (`audio_io/src/android.rs`). So the
`Processor` cannot be carried across a rebuild.

## Decision

**A new crate, `rust/crates/session` (`diapason_session`),** between `diapason_ffi` and
`engine`/`audio_io`:

```
diapason_ffi → session → engine → dsp
                       → audio_io
```

- **`Supervisor<B: AudioBackend>` is sans-IO.** Every decision happens in `tick(now_ns)`, with the
  time passed in. Tests drive it on virtual time through `OfflineBackend`, with no thread and no
  wall clock (`TESTING.md` §4).
- **`Session` is a thin driver.** One thread waits on an `mpsc` control channel with a 33 ms
  timeout, calls `tick` with the monotonic clock, and hands each snapshot to a subscriber. It has
  no FFI types, so the throttle lives in Rust (`ARCHITECTURE.md` §4) and the crate tests without
  Flutter.
- **Desired state is the source of truth.** The supervisor keeps what was asked for: running,
  input wanted, the tone. Every (re)open builds a fresh `Engine::prepare` pair and replays the
  desired state as `Command`s before the stream starts. This works whichever way the old
  `Processor` was lost.
- **The microphone is open only while it is wanted.** Changing "input wanted" rebuilds the stream
  with or without input. A metronome-only session therefore never shows the OS microphone
  indicator. M4 must make the metronome continue across that rebuild (the stream clock restarts at
  zero).
- **If the input fails to open, the stream opens without it.** The output keeps running (the
  metronome is unaffected, `PLATFORM_AUDIO.md` §2) and the input fault is reported. The input is
  not retried until it is asked for again, so a denied microphone cannot cause a rebuild loop.
- **Recovery** happens when `StreamHandle::disconnected` is set. The supervisor closes the stream
  and reopens it after 50 ms, doubling the wait each time up to 2 s. After 10 s without success it
  stops, in `Failed` with a typed fault. A successful recovery increments `rebuilds`, which is the
  snapshot's `route_changed` signal.
- **The panic denials of `adr/0021` apply to the whole crate.** No RT lint config applies, because
  nothing here runs on the audio thread. The crate documentation says so.

## Options rejected

- **A supervisor module inside `engine`.** It would need a carve-out from the RT lint
  configuration, so the configuration would stop meaning "this crate is RT-safe".
- **Supervising from Dart** (timers, platform-channel lifecycle). Rust owns audio end to end
  (`adr/0001`). Dart timers are not a clock this project trusts (`AGENTS.md` §6), and Dart cannot
  see `disconnected`.
- **Have `close` hand the callback back,** so the old `Processor` survives a rebuild. That changes
  every backend's contract, and it still fails when an open fails half-way, which drops the
  callback. Replay covers every case with one code path.

## Consequences

- `tools/check-deps.sh` enforces the edges: `session` depends only on `engine` and `audio_io`,
  nothing beneath it depends on `session`, and `diapason_ffi` reaches Rust only through `session`.
- Whatever the engine learns to do (tuner, metronome), the desired state grows to match. A command
  the supervisor does not replay is lost on the first route change. Every new `Command` needs a
  replay test.
- `ARCHITECTURE.md` §2 and `REPO_LAYOUT.md` list the crate. §4 gains the session thread, and its
  backpressure note now covers the hop to Dart. §7 names the session's `Fault` variants.
- `Session` is dropped by joining its thread, which first stops the stream: up to AAudio's 500 ms
  stop timeout. The app holds one session for its lifetime. Anything that ever drops one must not
  do it on a thread that cannot wait.

import 'package:meta/meta.dart';

/// Where the audio session is in its lifecycle (`docs/adr/0022`).
enum SessionState {
  /// No stream, and none wanted.
  stopped,

  /// A stream is open and running.
  running,

  /// The stream broke or would not open, and the engine is about to reopen it.
  recovering,

  /// The engine gave up. [SessionSnapshot.fault] says why; starting again retries.
  failed,
}

/// Why something failed, in the terms the UI offers a recovery for (`docs/ARCHITECTURE.md` §7).
enum AudioFault {
  /// The microphone permission is missing.
  permissionDenied,

  /// The device is gone, busy or broken. Worth retrying.
  deviceUnavailable,

  /// The device refused the configuration. Retrying the same request will not help.
  configurationUnsupported,

  /// A bug in this app, not in the device.
  internal,
}

/// The processing Android applied to the microphone (`docs/PLATFORM_AUDIO.md` §2).
enum InputPreset {
  /// No processing: what the tuner wants.
  unprocessed,

  /// Automatic gain off on most devices: the fallback.
  voiceRecognition,

  /// Anything else; [StreamDiagnostics.inputPresetCode] has the platform's value.
  other,
}

/// Which path one platform stream was actually given.
@immutable
class GrantedPath {
  /// Creates a granted path.
  const new({required this.lowLatency, required this.exclusive});

  /// The low-latency performance mode was granted.
  final bool lowLatency;

  /// Exclusive (MMAP) sharing was granted, rather than the shared mixer.
  final bool exclusive;

  @override
  bool operator ==(Object other) =>
      other is GrantedPath && other.lowLatency == lowLatency && other.exclusive == exclusive;

  @override
  int get hashCode => Object.hash(lowLatency, exclusive);

  @override
  String toString() => 'GrantedPath(lowLatency: $lowLatency, exclusive: $exclusive)';
}

/// What the diagnostics overlay and support reports need about the open stream. Counters describe
/// the current stream and restart from zero when it is rebuilt.
@immutable
class StreamDiagnostics {
  /// Creates diagnostics. Everything but [backend] defaults to "nothing known".
  const new({
    required this.backend,
    this.maxBlockFrames,
    this.framesPlayed = 0,
    this.callbacks = 0,
    this.worstCallback = Duration.zero,
    this.inputUnderruns = 0,
    this.xruns,
    this.framesPerBurst,
    this.inputPreset,
    this.inputPresetCode,
    this.outputPath,
    this.inputPath,
    this.lastError,
    this.commandsDropped = 0,
  });

  /// The backend's name, such as `aaudio`.
  final String backend;

  /// The largest block the audio callback may be given. `null` with no stream open.
  final int? maxBlockFrames;

  /// Frames the stream has played: the audio clock.
  final int framesPlayed;

  /// Audio callbacks completed.
  final int callbacks;

  /// The slowest audio callback so far. Budget: 15 % of the buffer period
  /// (`docs/AUDIO_ENGINE.md` §1).
  final Duration worstCallback;

  /// Blocks the microphone delivered late.
  final int inputUnderruns;

  /// Underruns and overruns the platform counted, where it counts them.
  final int? xruns;

  /// The output's burst size in frames, where the platform has one.
  final int? framesPerBurst;

  /// The microphone processing the device applied. `null` without a microphone.
  final InputPreset? inputPreset;

  /// The platform's value when [inputPreset] is [InputPreset.other].
  final int? inputPresetCode;

  /// The path the output was given.
  final GrantedPath? outputPath;

  /// The path the input was given. `null` without a microphone.
  final GrantedPath? inputPath;

  /// The most recent backend error, verbatim.
  final String? lastError;

  /// Commands the audio thread could not take. They are replayed on the next rebuild.
  final int commandsDropped;

  @override
  bool operator ==(Object other) =>
      other is StreamDiagnostics &&
      other.backend == backend &&
      other.maxBlockFrames == maxBlockFrames &&
      other.framesPlayed == framesPlayed &&
      other.callbacks == callbacks &&
      other.worstCallback == worstCallback &&
      other.inputUnderruns == inputUnderruns &&
      other.xruns == xruns &&
      other.framesPerBurst == framesPerBurst &&
      other.inputPreset == inputPreset &&
      other.inputPresetCode == inputPresetCode &&
      other.outputPath == outputPath &&
      other.inputPath == inputPath &&
      other.lastError == lastError &&
      other.commandsDropped == commandsDropped;

  @override
  int get hashCode => Object.hash(
    backend,
    maxBlockFrames,
    framesPlayed,
    callbacks,
    worstCallback,
    inputUnderruns,
    xruns,
    framesPerBurst,
    inputPreset,
    inputPresetCode,
    outputPath,
    inputPath,
    lastError,
    commandsDropped,
  );

  @override
  String toString() =>
      'StreamDiagnostics($backend, block: $maxBlockFrames, callbacks: $callbacks, '
      'worst: $worstCallback, xruns: $xruns, preset: $inputPreset, error: $lastError)';
}

/// The audio session at one moment, as the engine publishes it about 30 times a second.
///
/// Discrete fields ([state], [inputActive], [fault], …) belong in Riverpod state. [inputRms] is a
/// continuous signal: anything that animates it should listen to the stream directly rather than
/// rebuild widgets through a provider (`docs/ARCHITECTURE.md` §5).
@immutable
class SessionSnapshot {
  /// Creates a snapshot.
  const new({
    required this.state,
    required this.sampleRate,
    required this.diagnostics,
    this.inputActive = false,
    this.inputRms = 0,
    this.toneHz,
    this.rebuilds = 0,
    this.fault,
    this.inputFault,
  });

  /// Where the session is in its lifecycle.
  final SessionState state;

  /// The rate the stream runs at, in hertz.
  final int sampleRate;

  /// Whether the stream has the microphone.
  final bool inputActive;

  /// Input RMS over the last 50 ms, linear, 0 to 1.
  final double inputRms;

  /// The test tone's frequency while it plays.
  final double? toneHz;

  /// Streams rebuilt after the platform broke one: a route change or a lost device. Show a brief,
  /// calm indicator when it rises (`docs/PLATFORM_AUDIO.md` §2).
  final int rebuilds;

  /// Why the session is [SessionState.failed].
  final AudioFault? fault;

  /// Why the stream runs without the microphone it was asked for.
  final AudioFault? inputFault;

  /// The numbers the diagnostics overlay shows.
  final StreamDiagnostics diagnostics;

  /// Returns a copy with the given fields replaced. Nullable fields cannot be cleared through it;
  /// build a new snapshot for that.
  SessionSnapshot copyWith({
    SessionState? state,
    int? sampleRate,
    bool? inputActive,
    double? inputRms,
    double? toneHz,
    int? rebuilds,
    AudioFault? fault,
    AudioFault? inputFault,
    StreamDiagnostics? diagnostics,
  }) => SessionSnapshot(
    state: state ?? this.state,
    sampleRate: sampleRate ?? this.sampleRate,
    inputActive: inputActive ?? this.inputActive,
    inputRms: inputRms ?? this.inputRms,
    toneHz: toneHz ?? this.toneHz,
    rebuilds: rebuilds ?? this.rebuilds,
    fault: fault ?? this.fault,
    inputFault: inputFault ?? this.inputFault,
    diagnostics: diagnostics ?? this.diagnostics,
  );

  @override
  bool operator ==(Object other) =>
      other is SessionSnapshot &&
      other.state == state &&
      other.sampleRate == sampleRate &&
      other.inputActive == inputActive &&
      other.inputRms == inputRms &&
      other.toneHz == toneHz &&
      other.rebuilds == rebuilds &&
      other.fault == fault &&
      other.inputFault == inputFault &&
      other.diagnostics == diagnostics;

  @override
  int get hashCode => Object.hash(
    state,
    sampleRate,
    inputActive,
    inputRms,
    toneHz,
    rebuilds,
    fault,
    inputFault,
    diagnostics,
  );

  @override
  String toString() =>
      'SessionSnapshot($state, ${sampleRate}Hz, input: $inputActive, rms: $inputRms, '
      'tone: $toneHz, rebuilds: $rebuilds, fault: $fault, inputFault: $inputFault)';
}

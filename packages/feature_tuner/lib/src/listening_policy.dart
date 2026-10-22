/// What the stream should be doing for the tuner (`docs/PLATFORM_AUDIO.md` §5).
enum EngineTarget {
  /// No stream: the microphone is released and nothing plays.
  stopped,

  /// Output only, for the development test tone. The microphone is released.
  outputOnly,

  /// Microphone and output.
  duplex,
}

/// The tuner's whole lifecycle policy, as a pure function of what the user asked for and where the
/// app is: nothing listens unless the tuner is in front of the user.
///
/// - [listenWanted]: the user asked to listen, and has not stopped or left the tuner since.
/// - [toneWanted]: the user started the development A4 tone.
/// - [granted]: the microphone permission, as last read.
/// - [foreground]: the app is not hidden, paused or detached. An inactive app (behind the
///   permission dialog or the notification shade) is still in the foreground.
/// - [visible]: the tuner's route is the current one and its tickers run.
EngineTarget engineTarget({
  required bool listenWanted,
  required bool toneWanted,
  required bool granted,
  required bool foreground,
  required bool visible,
}) {
  if (!foreground || !visible) return EngineTarget.stopped;
  if (listenWanted && granted) return EngineTarget.duplex;
  if (toneWanted) return EngineTarget.outputOnly;
  return EngineTarget.stopped;
}

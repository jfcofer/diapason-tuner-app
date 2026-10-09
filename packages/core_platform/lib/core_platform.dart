/// Platform capabilities behind interfaces, so nothing above this layer imports a plugin.
///
/// Every implementation has a fake (`AGENTS.md` §5). `core_ui` widgets never call `dart:io` or a
/// plugin directly - they receive one of these instead, which is what keeps widget tests hermetic.
library;

export 'src/microphone_permission.dart';
export 'src/platform_microphone_permission.dart' show PlatformMicrophonePermission;

/// Test support for anything that implements `EngineHandle`.
///
/// A separate library so the app never imports it. It has no test-framework dependency, so both
/// `flutter_test` and `integration_test` can run the same contract.
library;

export 'src/testing/engine_contract.dart';

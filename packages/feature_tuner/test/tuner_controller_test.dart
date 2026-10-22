import 'dart:async';

import 'package:audio_engine/audio_engine.dart';
import 'package:core_domain/core_domain.dart';
import 'package:core_platform/core_platform.dart';
import 'package:fake_async/fake_async.dart';
import 'package:feature_tuner/feature_tuner.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

/// The controller over fakes, kept alive by a listener as the screen keeps it alive.
class Harness {
  new({MicrophonePermissionStatus initial = MicrophonePermissionStatus.notDetermined})
    : permission = FakeMicrophonePermission(initial: initial);

  final FakeAudioEngine engine = FakeAudioEngine();
  final FakeAppLifecycle lifecycle = FakeAppLifecycle();
  final FakeMicrophonePermission permission;

  late final ProviderContainer container = ProviderContainer(
    overrides: [
      engineHandleProvider.overrideWithValue(engine),
      microphonePermissionProvider.overrideWithValue(permission),
      appLifecycleProvider.overrideWithValue(lifecycle),
    ],
  )..listen(tunerControllerProvider, (_, _) {});

  TunerController get controller => container.read(tunerControllerProvider.notifier);
  TunerState get state => container.read(tunerControllerProvider);
  SessionSnapshot get session => engine.current;

  void background(FakeAsync async) {
    lifecycle
      ..set(AppLifecycleState.inactive)
      ..set(AppLifecycleState.hidden)
      ..set(AppLifecycleState.paused);
    async.flushMicrotasks();
  }

  void foreground(FakeAsync async) {
    lifecycle
      ..set(AppLifecycleState.hidden)
      ..set(AppLifecycleState.inactive)
      ..set(AppLifecycleState.resumed);
    async.flushMicrotasks();
  }
}

/// Runs [body] on virtual time with a fresh [Harness], disposing it afterwards.
void withTuner(
  void Function(Harness tuner, FakeAsync async) body, {
  MicrophonePermissionStatus initial = MicrophonePermissionStatus.notDetermined,
}) => fakeAsync((async) {
  final tuner = Harness(initial: initial);
  expect(tuner.state, const TunerState()); // builds it
  async.flushMicrotasks();
  body(tuner, async);
  tuner.container.dispose();
  async.flushMicrotasks();
});

void main() {
  test('the background releases the microphone, and the foreground restores it (L1, L2)', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      async.flushMicrotasks();
      expect(tuner.session.inputActive, isTrue);

      tuner.background(async);
      expect(tuner.session.state, SessionState.stopped);
      expect(tuner.state.listenWanted, isTrue, reason: 'the wish outlives the background');

      tuner.foreground(async);
      expect(tuner.session.inputActive, isTrue);
    });
  });

  test('the foreground does not start a tuner that was not listening', () {
    withTuner((tuner, async) {
      tuner
        ..background(async)
        ..foreground(async);
      expect(tuner.session.state, SessionState.stopped);
    }, initial: MicrophonePermissionStatus.granted);
  });

  test('an inactive app keeps listening: the permission dialog and the shade do that', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      async.flushMicrotasks();

      tuner.lifecycle.set(AppLifecycleState.inactive);
      async.flushMicrotasks();
      expect(tuner.session.inputActive, isTrue);

      tuner.lifecycle.set(AppLifecycleState.resumed);
      async.flushMicrotasks();
      expect(tuner.session.inputActive, isTrue);
    });
  });

  test('a permission revoked while away is read on return: no microphone, recovery shown', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      async.flushMicrotasks();

      tuner.background(async);
      tuner.permission.current = MicrophonePermissionStatus.permanentlyDenied;
      tuner.foreground(async);

      expect(tuner.session.state, SessionState.stopped);
      expect(tuner.state.permission, MicrophonePermissionStatus.permanentlyDenied);
    });
  });

  test('a grant made in Settings after a refusal starts listening on return', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      async.flushMicrotasks();
      expect(tuner.session.state, SessionState.stopped);

      unawaited(tuner.controller.openSettings());
      tuner.background(async);
      tuner.permission.current = MicrophonePermissionStatus.granted;
      tuner.foreground(async);

      expect(tuner.state.permission, MicrophonePermissionStatus.granted);
      expect(tuner.session.inputActive, isTrue);
    }, initial: MicrophonePermissionStatus.permanentlyDenied);
  });

  test('going off screen releases the microphone, and coming back does not reopen it', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      tuner.controller.toggleTestTone();
      async.flushMicrotasks();

      tuner.controller.setVisible(visible: false);
      expect(tuner.session.state, SessionState.stopped);
      expect(tuner.state.listenWanted, isFalse);
      expect(tuner.state.toneWanted, isFalse);

      tuner.controller.setVisible(visible: true);
      expect(tuner.session.state, SessionState.stopped);
    });
  });

  test('an answer that arrives after the tuner was left does not revive listening', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen()); // the dialog is up
      tuner.controller.setVisible(visible: false);
      async.flushMicrotasks(); // then answered

      tuner.controller.setVisible(visible: true);
      expect(tuner.state.listenWanted, isFalse);
      expect(tuner.session.state, SessionState.stopped);
    });
  });

  test('a return to the foreground that is already over does not reopen the microphone', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      async.flushMicrotasks();

      tuner.lifecycle
        ..set(AppLifecycleState.hidden)
        ..set(AppLifecycleState.resumed) // the permission is being read again…
        ..set(AppLifecycleState.hidden); // …when the app leaves once more
      async.flushMicrotasks();

      expect(tuner.session.state, SessionState.stopped);
    });
  });

  test('the tone plays without the microphone and survives the background', () {
    withTuner((tuner, async) {
      tuner.controller.toggleTestTone();
      expect(tuner.session.state, SessionState.running);
      expect(tuner.session.inputActive, isFalse);
      expect(tuner.session.toneHz, 440);

      tuner.background(async);
      expect(tuner.session.state, SessionState.stopped);

      tuner.foreground(async);
      expect(tuner.session.toneHz, 440);
      expect(tuner.session.inputActive, isFalse);
    });
  });

  test('stopping listening with the tone on keeps the output and drops the microphone', () {
    withTuner((tuner, async) {
      unawaited(tuner.controller.listen());
      tuner.controller.toggleTestTone();
      async.flushMicrotasks();
      expect(tuner.session.inputActive, isTrue);

      tuner.controller.stopListening();
      expect(tuner.session.state, SessionState.running);
      expect(tuner.session.inputActive, isFalse);
      expect(tuner.session.toneHz, 440);
    });
  });

  test('disposal stops the stream and the tone, so nothing replays it later', () {
    fakeAsync((async) {
      final tuner = Harness();
      unawaited(tuner.controller.listen());
      tuner.controller.toggleTestTone();
      async.flushMicrotasks();

      tuner.container.dispose();
      async.flushMicrotasks();
      expect(tuner.session.state, SessionState.stopped);

      tuner.engine.start(input: false);
      expect(tuner.session.toneHz, isNull);
    });
  });

  test('a refused microphone is told to the engine on every read', () {
    withTuner((tuner, async) {
      tuner.engine.start(input: true);
      expect(tuner.session.inputFault, AudioFault.permissionDenied);
    }, initial: MicrophonePermissionStatus.denied);
  });
}

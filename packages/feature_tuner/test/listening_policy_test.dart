import 'package:feature_tuner/src/listening_policy.dart';
import 'package:flutter_test/flutter_test.dart';

/// One combination of the policy's five inputs.
typedef Inputs = ({
  bool listenWanted,
  bool toneWanted,
  bool granted,
  bool foreground,
  bool visible,
});

/// All 32 combinations: the policy is small enough to check exhaustively.
final List<Inputs> everyCombination = [
  for (var bits = 0; bits < 32; bits++)
    (
      listenWanted: bits & 1 != 0,
      toneWanted: bits & 2 != 0,
      granted: bits & 4 != 0,
      foreground: bits & 8 != 0,
      visible: bits & 16 != 0,
    ),
];

EngineTarget targetFor(Inputs i) => engineTarget(
  listenWanted: i.listenWanted,
  toneWanted: i.toneWanted,
  granted: i.granted,
  foreground: i.foreground,
  visible: i.visible,
);

void main() {
  test('in the background, nothing runs, whatever was asked for (L1, L8)', () {
    for (final inputs in everyCombination.where((i) => !i.foreground)) {
      expect(targetFor(inputs), EngineTarget.stopped, reason: '$inputs');
    }
  });

  test('off the tuner, nothing runs, whatever was asked for', () {
    for (final inputs in everyCombination.where((i) => !i.visible)) {
      expect(targetFor(inputs), EngineTarget.stopped, reason: '$inputs');
    }
  });

  test('the microphone opens only when asked for, permitted, in front and on screen', () {
    final duplex = everyCombination.where((i) => targetFor(i) == EngineTarget.duplex);

    expect(duplex.every((i) => i.listenWanted && i.granted && i.foreground && i.visible), isTrue);
    expect(duplex, hasLength(2), reason: 'with and without the tone');
  });

  test('the tone plays without the microphone when listening is off or refused', () {
    for (final inputs in everyCombination.where(
      (i) => i.toneWanted && i.foreground && i.visible && !(i.listenWanted && i.granted),
    )) {
      expect(targetFor(inputs), EngineTarget.outputOnly, reason: '$inputs');
    }
  });

  test('with nothing asked for, nothing runs', () {
    for (final inputs in everyCombination.where((i) => !i.listenWanted && !i.toneWanted)) {
      expect(targetFor(inputs), EngineTarget.stopped, reason: '$inputs');
    }
  });
}

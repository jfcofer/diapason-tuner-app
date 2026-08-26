import 'package:diapason/bootstrap.dart';

/// Entrypoint for the `prod` flavour.
///
/// Flavour configuration comes from `--dart-define-from-file=flavors/prod.json`; see
/// `just run android prod`.
void main() => bootstrap();

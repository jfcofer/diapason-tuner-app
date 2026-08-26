/// Pure Dart domain model for Diapason.
///
/// This package has **no Flutter import, no I/O and no plugins** (`AGENTS.md` §5), which is what
/// lets it be tested with plain `dart test` and reused unchanged by anything above it.
///
/// Note and cents math is intentionally duplicated between here and `rust/crates/dsp`, kept honest
/// by a shared fixture rather than by a shared implementation - see `docs/adr/0007`.
library;

export 'src/engine_status.dart';

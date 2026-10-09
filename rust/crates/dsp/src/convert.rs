//! Every lossy numeric conversion in the project, and why each loss is acceptable (`adr/0018`).
//!
//! `clippy::pedantic` rightly flags narrowing casts. Some cannot be avoided in DSP, so they live
//! here and nowhere else: each under a scoped `#[expect]` that fails the build if the lint ever
//! stops firing. Add a function here rather than an `as` anywhere else.

/// Narrow an `f64` to `f32`.
///
/// DSP state that accumulates (phase, sums of squares) is kept in `f64` so that rounding cannot
/// build up over a long stream; what leaves it (samples, levels, parameters) is `f32` by contract
/// (`docs/PLATFORM_AUDIO.md` §1). Values beyond `f32` range become infinite, which no audio value
/// reaches.
///
/// ```
/// assert_eq!(diapason_dsp::convert::to_f32(0.5), 0.5_f32);
/// ```
#[expect(
    clippy::cast_possible_truncation,
    reason = "f32 is the audio sample and parameter type by contract; f64 is internal precision"
)]
#[must_use]
#[inline]
pub fn to_f32(value: f64) -> f32 {
    value as f32
}

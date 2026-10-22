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

/// Widen a sample index or lag to `f64`.
///
/// Exact on every target we ship. On 32-bit ones (`armeabi-v7a`, `docs/adr/0020`) every `usize`
/// fits in `f64`'s 53-bit significand. On 64-bit ones the conversion is exact below 2⁵³, and every
/// index and lag here is far below that: they are bounded by the analysis window (at most 2²⁰
/// samples, see [`crate::pitch::MAX_WINDOW`]) or by a fixture's length. The lint fires because it
/// cannot know the bound.
///
/// ```
/// use diapason_dsp::convert::index_to_f64;
/// assert_eq!(index_to_f64(4_096), 4_096.0);
/// // The largest value a 32-bit usize can hold converts exactly on any target.
/// assert_eq!(index_to_f64(usize::try_from(u32::MAX).unwrap()), 4_294_967_295.0);
/// ```
#[expect(
    clippy::cast_precision_loss,
    reason = "indices and lags are far below 2^53, where usize to f64 is exact"
)]
#[must_use]
#[inline]
pub fn index_to_f64(index: usize) -> f64 {
    index as f64
}

/// Truncate a count held in an `f64` to an index: towards zero, saturating at both ends.
///
/// Negative values and NaN give 0; values beyond `usize::MAX` give `usize::MAX`. That is what Rust's
/// float-to-integer `as` does, so the only loss is the fraction, which a count derived from a ratio
/// (harmonics below Nyquist, say) is meant to drop. Callers that need a bound clamp the result.
///
/// ```
/// use diapason_dsp::convert::index_from_f64;
/// assert_eq!(index_from_f64(19.9), 19);
/// assert_eq!(index_from_f64(-3.0), 0);
/// assert_eq!(index_from_f64(f64::NAN), 0);
/// assert_eq!(index_from_f64(1e300), usize::MAX);
/// ```
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "truncation towards zero and saturation are the documented behaviour of this function"
)]
#[must_use]
#[inline]
pub fn index_from_f64(value: f64) -> usize {
    value as usize
}

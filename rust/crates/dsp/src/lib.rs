//! Pure computation. No I/O, no allocation in hot paths, no platform code, no dependency on
//! `audio_io`. Everything here must stay testable offline against fixture buffers.
//!
//! What exists so far is what the audio spine needs (`T-002a`): a phase-continuous test tone and an
//! input level meter. Pitch detection arrives in `T-003` (`docs/adr/0005`). Every lossy numeric
//! conversion in the project lives in [`convert`] (`docs/adr/0018`).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

pub mod convert;
pub mod level;
pub mod osc;

/// Identifies the DSP build, so the value crossing the FFI boundary in `T-001` originates from the
/// bottom of the Rust stack rather than from the binding layer. Replaced by real signal processing
/// in `T-003`; it exists to prove the dependency direction, not to be useful.
///
/// ```
/// assert!(diapason_dsp::build_id().starts_with("diapason_dsp"));
/// ```
#[must_use]
pub fn build_id() -> String {
    concat!("diapason_dsp ", env!("CARGO_PKG_VERSION")).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_id_names_the_crate_and_version() {
        assert_eq!(
            build_id(),
            concat!("diapason_dsp ", env!("CARGO_PKG_VERSION"))
        );
    }
}

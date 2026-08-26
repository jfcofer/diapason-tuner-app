//! Pure computation. No I/O, no allocation in hot paths, no platform code, no dependency on
//! `audio_io`. Everything here must stay testable offline against fixture buffers.
//!
//! `T-001` deliberately ships **no DSP**. This crate exists now so that the workspace, the lint
//! configuration and the test harness are real and enforced from the first commit; the pitch
//! detection it is named for arrives in `T-003` (`docs/adr/0005`).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

/// Identifies the DSP build, so the value crossing the FFI boundary in `T-001` originates from the
/// bottom of the Rust stack rather than from the binding layer. Replaced by real signal processing
/// in `T-003`; it exists to prove the dependency direction, not to be useful.
///
/// ```
/// assert!(diapason_dsp::build_id().starts_with("diapason_dsp"));
/// ```
#[must_use]
pub fn build_id() -> String {
    format!("diapason_dsp {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_id_names_the_crate_and_version() {
        assert_eq!(
            build_id(),
            format!("diapason_dsp {}", env!("CARGO_PKG_VERSION"))
        );
    }
}

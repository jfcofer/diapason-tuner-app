//! Pure computation. No I/O, no allocation in hot paths, no platform code, no dependency on
//! `audio_io`. Everything here must stay testable offline against fixture buffers.
//!
//! - [`pitch`]: the `McLeod` Pitch Method (MPM) detector (`docs/AUDIO_ENGINE.md` §3,
//!   `docs/adr/0005`).
//! - [`filter`]: the pre-processing in front of it (`docs/AUDIO_ENGINE.md` §2): a DC block, and
//!   the FIR low-pass that band-limits or decimates.
//! - [`synth`]: deterministic, seeded fixture signals that test and bench it.
//! - [`osc`] and [`level`]: what the audio spine needed first (`T-002a`), a phase-continuous test
//!   tone and an input level meter.
//! - [`convert`]: every lossy numeric conversion in the project (`docs/adr/0018`).
//!
//! Release builds abort on a panic (`docs/adr/0021`), and this crate is the real-time path, so the
//! panicking constructs are denied outside tests: what can fail returns an `Option` or a `Result`.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

pub mod convert;
pub mod filter;
pub mod level;
pub mod osc;
pub mod pitch;
pub mod synth;

/// Identifies the DSP build, so the value crossing the FFI boundary in `T-001` originates from the
/// bottom of the Rust stack rather than from the binding layer. The FFI round-trip test still calls
/// it to prove the dependency direction; it carries no signal processing.
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

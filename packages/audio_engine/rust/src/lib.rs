// The hand-written API surface is panic-free by construction: release builds abort on panic, so a
// panic here would end the app rather than reach Dart as an error (docs/adr/0021). Scoped to `api`
// because the generated module unwraps inside flutter_rust_bridge's own wire decoder.
#[deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
pub mod api;
mod frb_generated;

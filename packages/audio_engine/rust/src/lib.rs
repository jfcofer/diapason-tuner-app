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

// Debug builds of the app trap any allocation inside `assert_no_alloc`, which wraps every audio
// callback, so an allocation on the real-time path aborts the debug app at the line that made it
// (docs/AUDIO_ENGINE.md §7). It sees only Rust's allocator. Release builds keep the system one.
#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

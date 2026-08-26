//! Repository automation that is easier to write in Rust than in bash.
//!
//! Invoked as `cargo xtask <command>`. Fixture generation, bench comparison and size reporting land
//! here as the tasks that need them arrive; `T-001` ships the dispatcher so the entry point exists
//! and is covered by `cargo clippy` and `cargo build` from the first commit.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let command = std::env::args().nth(1);
    match command.as_deref() {
        Some("size-report") => {
            eprintln!("xtask: size-report is not implemented yet (T-0xx, milestone M5)");
            ExitCode::FAILURE
        }
        Some("fixtures") => {
            eprintln!("xtask: fixtures is not implemented yet (T-003)");
            ExitCode::FAILURE
        }
        Some(other) => {
            eprintln!("xtask: unknown command `{other}`");
            usage();
            ExitCode::FAILURE
        }
        None => {
            usage();
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("usage: cargo xtask <fixtures|size-report>");
}

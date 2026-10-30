//! Repository automation that is easier to write in Rust than in bash.
//!
//! Invoked as `cargo xtask <command>` (the alias is in `.cargo/config.toml`). `fixtures` imports,
//! measures and checks the recorded fixtures (`docs/adr/0030`); size reporting lands with M5.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

use std::process::ExitCode;

mod fixtures;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("size-report") => {
            eprintln!("xtask: size-report is not implemented yet (T-0xx, milestone M5)");
            ExitCode::FAILURE
        }
        Some("fixtures") => fixtures::run(&args[1..]),
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

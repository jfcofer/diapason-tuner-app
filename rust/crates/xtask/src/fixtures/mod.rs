//! Recorded fixtures (`docs/adr/0030`, `T-003e`): import a note from a source archive, measure its
//! reference pitch, and check what is committed. Synthetic fixtures are generated in code
//! (`docs/adr/0027`) and never pass through here.

mod check;
mod import;
mod manifest;
mod reference;

use std::collections::BTreeMap;
use std::process::ExitCode;

/// `cargo xtask fixtures <import|reference|check> [--key value]…`
pub fn run(args: &[String]) -> ExitCode {
    let result = match args.first().map(String::as_str) {
        Some("import") => Args::parse(&args[1..]).and_then(|a| import::run(&a)),
        Some("reference") => Args::parse(&args[1..]).and_then(|a| reference::run(&a)),
        Some("check") => {
            let errors = check::run(&manifest::dir());
            if errors.is_empty() {
                println!("✓ recorded fixtures match their manifest, licences and caps");
                Ok(())
            } else {
                Err(errors
                    .iter()
                    .map(|e| format!("  ✗ {e}"))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
        }
        _ => Err(
            "usage: cargo xtask fixtures <import|reference|check> (see fixtures/audio/README.md)"
                .into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask fixtures: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--key value` pairs.
pub struct Args(BTreeMap<String, String>);

impl Args {
    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut map = BTreeMap::new();
        let mut it = raw.iter();
        while let Some(key) = it.next() {
            let key = key
                .strip_prefix("--")
                .ok_or_else(|| format!("expected --key, got `{key}`"))?;
            let value = it.next().ok_or_else(|| format!("--{key} needs a value"))?;
            map.insert(key.to_owned(), value.clone());
        }
        Ok(Self(map))
    }

    fn get(&self, key: &str) -> Result<&str, String> {
        self.optional(key).ok_or_else(|| format!("missing --{key}"))
    }

    fn optional(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn number(&self, key: &str) -> Result<u64, String> {
        self.get(key)?
            .parse()
            .map_err(|_| format!("--{key}: expected a whole number"))
    }
}

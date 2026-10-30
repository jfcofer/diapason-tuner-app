//! `fixtures/audio/manifest.json`: where each recording came from, under which licence, and the
//! pitch it is held to (`docs/adr/0030`). The schema's one home; `dsp`'s test reads a subset.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The manifest, as committed.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Source libraries by short id. A library's licence is a fact about the library, not the note.
    pub sources: BTreeMap<String, Source>,
    /// Fixtures, sorted by id.
    pub fixtures: Vec<Fixture>,
}

/// A published sample library, pinned to the archive a fixture was cut from.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub library: String,
    pub version: String,
    /// Where the archive is downloaded from, by HTTPS.
    pub url: String,
    /// The archive's file name, as downloaded.
    pub archive: String,
    /// Filled by the first `import` from the archive, and checked by every later one.
    #[serde(default)]
    pub archive_sha256: String,
    /// An SPDX id from [`super::check::LICENCES`].
    pub licence: String,
    /// The licence file copied verbatim from the archive, relative to `fixtures/audio/`.
    pub evidence: String,
    /// The evidence file's SHA-256, so a changed byte (a line ending, say) shows.
    pub evidence_sha256: String,
    /// The attribution a CC-BY licence requires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit: Option<String>,
}

/// What a fixture is held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Set {
    /// Asserted against a budget in `docs/AUDIO_ENGINE.md` §1. Needs a reference within
    /// [`super::check::GOLD_UNCERTAINTY_CENTS`].
    Gold,
    /// Measured under a regression ceiling until a budget exists (`T-003g`).
    Ceiling,
    /// Measured and reported, never gated: vibrato, noise, a pitch that will not hold still.
    Stress,
}

/// Which channel of the source became the fixture. Never a mix: summing two microphones or
/// pickups comb-filters the partials the detector reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Mono,
    Left,
    Right,
}

/// One recorded note.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub id: String,
    /// The WAV, relative to `fixtures/audio/`.
    pub file: String,
    /// A key of [`Manifest::sources`].
    pub source: String,
    /// The path inside the source archive.
    pub member: String,
    pub member_sha256: String,
    pub channel: Channel,
    /// Where the cut starts in the source member, and how long it is, in frames.
    pub start_frame: u64,
    pub frames: u64,
    pub instrument: String,
    pub variant: String,
    pub set: Set,
    /// The note as measured, in scientific pitch notation (A4 = 440 Hz). Not the library's label:
    /// Karoryfer's bass names are an octave high.
    pub note: String,
    pub sample_rate: u32,
    pub bits: u16,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<Reference>,
}

/// The independently measured pitch, from `cargo xtask fixtures reference`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// The median harmonic consensus over the window: the amplitude-weighted mean of `f_k / k`.
    pub hz: f64,
    /// The first partial alone, for comparison: a guitar's body can pull it off the series.
    pub first_partial_hz: f64,
    /// The window the statistic covers, in milliseconds from the start of the file. The detector's
    /// test takes its median over the same span.
    pub window_ms: [u32; 2],
    pub method: String,
    /// The standard error of the median, after removing the window's linear glide.
    pub uncertainty_cents: f64,
    /// How far the consensus moves across the window, by the linear fit. Plucked strings go
    /// flat as they decay; this is the instrument, not measurement error.
    pub glide_cents: f64,
}

/// `fixtures/audio/` in this checkout.
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/audio")
}

/// Read the manifest at `root/manifest.json`.
pub fn load(root: &Path) -> Result<Manifest, String> {
    let path = root.join("manifest.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write the manifest back, fixtures sorted by id, so a diff shows only what changed.
pub fn save(root: &Path, manifest: &mut Manifest) -> Result<(), String> {
    manifest.fixtures.sort_by(|a, b| a.id.cmp(&b.id));
    let path = root.join("manifest.json");
    let mut text = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(bytes))
}

/// Lowercase hex of a digest.
pub fn hex(digest: &[u8]) -> String {
    use std::fmt::Write;
    digest
        .iter()
        .fold(String::with_capacity(2 * digest.len()), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

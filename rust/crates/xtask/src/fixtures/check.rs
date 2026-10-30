//! `cargo xtask fixtures check`, in `just verify`: every committed recording is what the manifest
//! says it is, under a licence `docs/adr/0030` permits, inside the size caps.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::manifest::{self, Fixture, Manifest, Set, Source};
use super::reference;

/// Licences a checked-in test asset may carry (`docs/adr/0030`). `LicenseRef-unrestricted` is a
/// publisher's explicit grant of use without conditions, such as the University of Iowa's.
pub const LICENCES: [&str; 3] = ["CC0-1.0", "CC-BY-4.0", "LicenseRef-unrestricted"];
/// A gold fixture's reference must be at least this precise: a quarter of §1's ±2-cent target.
pub const GOLD_UNCERTAINTY_CENTS: f64 = 0.5;
const MAX_FILE_BYTES: u64 = 1 << 20;
const MAX_TOTAL_BYTES: u64 = 10 << 20;
/// How far a stored reference may sit from a fresh measurement: the manifest's rounding (four
/// decimals of a hertz, 0.003 ¢ at 30 Hz), with room for another platform's `sin` and `cos`.
const REDERIVE_CENTS: f64 = 0.01;

/// Every failure, so one run reports them all.
pub fn run(root: &Path) -> Vec<String> {
    let manifest = match manifest::load(root) {
        Ok(m) => m,
        Err(e) => return vec![e],
    };
    let mut errors = integrity(root, &manifest);
    errors.extend(coverage(&manifest));
    if errors.is_empty() {
        errors.extend(rederive(root, &manifest));
    }
    errors
}

/// Hashes, formats, licence evidence, sizes, and which set each instrument is in.
pub fn integrity(root: &Path, manifest: &Manifest) -> Vec<String> {
    let mut errors = Vec::new();
    for (id, source) in &manifest.sources {
        let used = manifest.fixtures.iter().any(|f| f.source == *id);
        check_source(root, id, source, used, &mut errors);
    }
    let mut total = 0;
    let mut ids = BTreeSet::new();
    for f in &manifest.fixtures {
        if !ids.insert(f.id.as_str()) {
            errors.push(format!("fixture {}: duplicate id", f.id));
        }
        total += check_fixture(root, manifest, f, &mut errors);
    }
    if total > MAX_TOTAL_BYTES {
        errors.push(format!("fixtures total {total} bytes, over the 10 MiB cap"));
    }
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_wav = Path::new(&name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
            let stem = name
                .rsplit_once('.')
                .map_or(name.as_str(), |(stem, _)| stem);
            if is_wav && !ids.contains(stem) {
                errors.push(format!("{name} is not in the manifest"));
            }
        }
    }
    errors
}

fn check_source(root: &Path, id: &str, source: &Source, used: bool, errors: &mut Vec<String>) {
    if !LICENCES.contains(&source.licence.as_str()) {
        errors.push(format!(
            "source {id}: licence {} is not one adr/0030 permits",
            source.licence
        ));
    }
    if source.licence == "CC-BY-4.0" && source.credit.is_none() {
        errors.push(format!("source {id}: CC-BY-4.0 needs a credit line"));
    }
    if !source.url.starts_with("https://") {
        errors.push(format!("source {id}: url must be https"));
    }
    // Evidence is a file directly under LICENSES/, stored byte for byte (`.gitattributes`: -text).
    let name = source.evidence.strip_prefix("LICENSES/").unwrap_or("");
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        errors.push(format!(
            "source {id}: evidence must be LICENSES/<file>, not {}",
            source.evidence
        ));
    } else {
        match fs::read(root.join(&source.evidence)) {
            Ok(bytes) if !bytes.is_empty() => {
                if manifest::sha256_hex(&bytes) != source.evidence_sha256 {
                    errors.push(format!(
                        "source {id}: {} does not match its sha256",
                        source.evidence
                    ));
                }
            }
            _ => errors.push(format!(
                "source {id}: licence evidence {} is missing",
                source.evidence
            )),
        }
    }
    if used && !is_sha256(&source.archive_sha256) {
        errors.push(format!(
            "source {id}: fixtures are cut from it, but its archive is not pinned"
        ));
    }
}

/// Checks one fixture; returns its size.
fn check_fixture(root: &Path, manifest: &Manifest, f: &Fixture, errors: &mut Vec<String>) -> u64 {
    let label = format!("fixture {}", f.id);
    if !manifest.sources.contains_key(&f.source) {
        errors.push(format!("{label}: unknown source {}", f.source));
    }
    if f.file != format!("{}.wav", f.id) || f.id.contains(['/', '\\', '.']) {
        errors.push(format!(
            "{label}: its file must be {}.wav, in fixtures/audio/",
            f.id
        ));
        return 0;
    }
    if !is_sha256(&f.member_sha256) {
        errors.push(format!("{label}: the source member's sha256 is not pinned"));
    }
    // adr/0030: acoustic guitar, and only it, is held to §1. A gold note cannot leave the set to
    // pass, nor another instrument join it unbudgeted.
    if (f.instrument == "acoustic-guitar") != (f.set == Set::Gold) {
        errors.push(format!(
            "{label}: acoustic guitar is gold, and gold is acoustic guitar"
        ));
    }
    let Ok(bytes) = fs::read(root.join(&f.file)) else {
        errors.push(format!("{label}: {} is missing", f.file));
        return 0;
    };
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > MAX_FILE_BYTES {
        errors.push(format!("{label}: {size} bytes, over the 1 MiB cap"));
    }
    if manifest::sha256_hex(&bytes) != f.sha256 {
        errors.push(format!("{label}: {} does not match its sha256", f.file));
    }
    match hound::WavReader::new(std::io::Cursor::new(&bytes)) {
        Err(e) => errors.push(format!("{label}: not a WAV: {e}")),
        Ok(reader) => {
            let spec = reader.spec();
            if spec.channels != 1
                || spec.sample_format != hound::SampleFormat::Int
                || spec.sample_rate != f.sample_rate
                || spec.bits_per_sample != f.bits
                || u64::from(reader.duration()) != f.frames
            {
                errors.push(format!(
                    "{label}: the WAV header disagrees with the manifest"
                ));
            }
            if f.sample_rate != 44_100 && f.sample_rate != 48_000 {
                errors.push(format!(
                    "{label}: {} Hz; fixtures are 44.1 or 48 kHz",
                    f.sample_rate
                ));
            }
        }
    }
    match (&f.reference, f.set) {
        (None, _) => errors.push(format!("{label}: no reference pitch")),
        (Some(r), Set::Gold) if r.uncertainty_cents > GOLD_UNCERTAINTY_CENTS => {
            errors.push(format!(
                "{label}: reference uncertainty {} cents, over gold's {GOLD_UNCERTAINTY_CENTS}",
                r.uncertainty_cents
            ));
        }
        _ => {}
    }
    size
}

/// The stored references are what `reference` measures from the committed WAVs today: a hand
/// edit to a pitch, or a change to the method, shows here.
pub fn rederive(root: &Path, manifest: &Manifest) -> Vec<String> {
    let mut errors = Vec::new();
    for f in &manifest.fixtures {
        let Some(stored) = &f.reference else { continue };
        let measured = reference::read_mono(&root.join(&f.file)).and_then(|(samples, rate)| {
            reference::measure(
                &samples,
                rate,
                reference::note_hz(&f.note)?,
                stored.window_ms,
            )
        });
        match measured {
            Err(e) => errors.push(format!(
                "fixture {}: reference does not re-derive: {e}",
                f.id
            )),
            Ok(r) => {
                let off = |a: f64, b: f64| reference::cents(a, b).abs() > REDERIVE_CENTS;
                if off(r.hz, stored.hz) || off(r.first_partial_hz, stored.first_partial_hz) {
                    errors.push(format!(
                        "fixture {}: stored reference {} Hz, re-derived {} Hz",
                        f.id, stored.hz, r.hz
                    ));
                }
            }
        }
    }
    errors
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// The instruments T-003e must cover, as `(instrument, set, note)`; `None` matches any note.
const COVERAGE: [(&str, Set, Option<&str>); 6] = [
    ("acoustic-guitar", Set::Gold, None),
    ("electric-guitar", Set::Ceiling, None),
    ("electric-bass", Set::Ceiling, Some("B0")),
    ("electric-bass", Set::Ceiling, Some("E1")),
    ("ukulele", Set::Ceiling, None),
    ("violin", Set::Ceiling, None),
];

/// Each required instrument, set and note has at least one fixture.
pub fn coverage(manifest: &Manifest) -> Vec<String> {
    COVERAGE
        .iter()
        .filter(|(instrument, set, note)| {
            !manifest.fixtures.iter().any(|f| {
                f.instrument == *instrument && f.set == *set && note.is_none_or(|n| f.note == n)
            })
        })
        .map(|(instrument, set, note)| {
            format!(
                "no {set:?} {instrument} fixture{}",
                note.map(|n| format!(" at {n}")).unwrap_or_default()
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::manifest::{Channel, Fixture, Reference, Source};

    /// A throwaway `fixtures/audio/` with one synthetic WAV. The WAV only exercises the checker; it
    /// is never a fixture.
    fn scratch(name: &str) -> (std::path::PathBuf, Manifest) {
        let root = std::env::temp_dir().join(format!(
            "diapason-fixtures-check-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("LICENSES")).unwrap();
        fs::write(root.join("LICENSES/lib.txt"), "CC0 1.0 Universal\n").unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(root.join("tone.wav"), spec).unwrap();
        for i in 0..4_800 {
            w.write_sample((i % 97) * 1000).unwrap();
        }
        w.finalize().unwrap();
        let sha256 = manifest::sha256_hex(&fs::read(root.join("tone.wav")).unwrap());
        let mut manifest = Manifest::default();
        manifest.sources.insert(
            "lib".into(),
            Source {
                library: "Lib".into(),
                version: "1".into(),
                url: "https://example.org/lib".into(),
                archive: "lib.zip".into(),
                archive_sha256: "a".repeat(64),
                licence: "CC0-1.0".into(),
                evidence: "LICENSES/lib.txt".into(),
                evidence_sha256: manifest::sha256_hex(b"CC0 1.0 Universal\n"),
                credit: None,
            },
        );
        manifest.fixtures.push(Fixture {
            id: "tone".into(),
            file: "tone.wav".into(),
            source: "lib".into(),
            member: "tone.wav".into(),
            member_sha256: "b".repeat(64),
            channel: Channel::Mono,
            start_frame: 0,
            frames: 4_800,
            instrument: "acoustic-guitar".into(),
            variant: "steel".into(),
            set: Set::Gold,
            note: "A4".into(),
            sample_rate: 48_000,
            bits: 24,
            sha256,
            reference: Some(Reference {
                hz: 440.0,
                first_partial_hz: 440.0,
                window_ms: [0, 100],
                method: "test".into(),
                uncertainty_cents: 0.1,
                glide_cents: 0.0,
            }),
        });
        (root, manifest)
    }

    #[test]
    fn a_consistent_directory_passes() {
        let (root, manifest) = scratch("ok");
        assert_eq!(integrity(&root, &manifest), Vec::<String>::new());
    }

    #[test]
    fn a_flipped_byte_fails() {
        let (root, manifest) = scratch("flip");
        let mut bytes = fs::read(root.join("tone.wav")).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(root.join("tone.wav"), bytes).unwrap();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("sha256"))
        );
    }

    #[test]
    fn a_missing_licence_file_fails() {
        let (root, manifest) = scratch("licence");
        fs::remove_file(root.join("LICENSES/lib.txt")).unwrap();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("licence evidence"))
        );
    }

    #[test]
    fn a_licence_outside_the_list_fails() {
        let (root, mut manifest) = scratch("gpl");
        manifest.sources.get_mut("lib").unwrap().licence = "GPL-3.0-or-later".into();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("GPL"))
        );
    }

    #[test]
    fn an_unlisted_wav_fails() {
        let (root, manifest) = scratch("orphan");
        fs::copy(root.join("tone.wav"), root.join("stray.wav")).unwrap();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("stray.wav"))
        );
    }

    #[test]
    fn an_imprecise_gold_reference_fails() {
        let (root, mut manifest) = scratch("imprecise");
        manifest.fixtures[0]
            .reference
            .as_mut()
            .unwrap()
            .uncertainty_cents = 0.8;
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("uncertainty"))
        );
    }

    #[test]
    fn a_changed_licence_byte_fails() {
        let (root, manifest) = scratch("crlf");
        fs::write(root.join("LICENSES/lib.txt"), "CC0 1.0 Universal\r\n").unwrap();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("lib.txt does not match"))
        );
    }

    #[test]
    fn paths_outside_the_directory_fail() {
        let (root, mut manifest) = scratch("paths");
        manifest.sources.get_mut("lib").unwrap().evidence = "../manifest.json".into();
        manifest.fixtures[0].file = "../tone.wav".into();
        let errors = integrity(&root, &manifest);
        assert!(
            errors.iter().any(|e| e.contains("LICENSES/<file>")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("tone.wav, in fixtures/audio")),
            "{errors:?}"
        );
    }

    #[test]
    fn unpinned_provenance_fails() {
        let (root, mut manifest) = scratch("unpinned");
        manifest.sources.get_mut("lib").unwrap().archive_sha256 = String::new();
        manifest.fixtures[0].member_sha256 = String::new();
        let errors = integrity(&root, &manifest);
        assert!(
            errors.iter().any(|e| e.contains("archive is not pinned")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("member's sha256")),
            "{errors:?}"
        );
    }

    /// A gold note moved to another set to escape §1 fails, and so does gold another instrument.
    #[test]
    fn gold_is_exactly_acoustic_guitar() {
        let (root, mut manifest) = scratch("gold");
        manifest.fixtures[0].set = Set::Stress;
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("gold is acoustic guitar"))
        );
        manifest.fixtures[0].set = Set::Gold;
        manifest.fixtures[0].instrument = "ukulele".into();
        assert!(
            integrity(&root, &manifest)
                .iter()
                .any(|e| e.contains("gold is acoustic guitar"))
        );
    }

    /// A hand-edited pitch does not survive re-derivation.
    #[test]
    fn a_hand_edited_reference_fails() {
        let (root, mut manifest) = scratch("rederive");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(root.join("tone.wav"), spec).unwrap();
        for i in 0..96_000 {
            let t = f64::from(i) / 48_000.0;
            let v = (std::f64::consts::TAU * 440.0 * t).sin() * f64::from(1 << 22);
            // Offset to a positive count, convert, then back: no lossy cast outside dsp::convert.
            let biased = diapason_dsp::convert::index_from_f64(v + f64::from(1 << 23));
            w.write_sample(i32::try_from(biased).unwrap() - (1 << 23))
                .unwrap();
        }
        w.finalize().unwrap();
        let r = manifest.fixtures[0].reference.as_mut().unwrap();
        r.window_ms = [300, 1300];
        r.hz = 440.0;
        r.first_partial_hz = 440.0;
        assert_eq!(rederive(&root, &manifest), Vec::<String>::new());
        manifest.fixtures[0].reference.as_mut().unwrap().hz = 440.2;
        assert!(
            rederive(&root, &manifest)
                .iter()
                .any(|e| e.contains("re-derived"))
        );
    }

    #[test]
    fn missing_instruments_are_named() {
        let (_, manifest) = scratch("coverage");
        let missing = coverage(&manifest);
        assert!(missing.iter().any(|e| e.contains("B0")), "{missing:?}");
    }
}

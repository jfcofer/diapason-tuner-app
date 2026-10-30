//! The tuner's chain on recorded notes (`docs/AUDIO_ENGINE.md` §1, `docs/TESTING.md` §2,
//! `docs/adr/0030`).
//!
//! Each fixture in `fixtures/audio/manifest.json` is streamed through the shipped chain at its
//! native rate, glued as the engine will glue it (`common::Tuner`: `chain::Front`, then
//! `chain::Analysis`), from the first sample of the file, under `Selection::Auto` from the home
//! range its instrument's preset would pick ([`home`]). Nothing is resampled or normalised.
//!
//! **The truth** is the manifest's `reference.hz`: the harmonic consensus over
//! `reference.window_ms`, the amplitude-weighted mean of `f_k / k` over the first six partials,
//! measured by `cargo xtask fixtures reference` with phase advance, which shares no code with
//! `Mpm` (owner, 2026-10-30). Against the first partial alone, which a guitar's body pulls off the
//! series, five of the nine gold notes missed ±2 ¢; the manifest keeps it as `first_partial_hz`.
//!
//! **Which frames.** A frame counts when every input sample it depends on lies inside the
//! reference window: `start ≤ end − span` and `end ≤ stop`, where `end` is [`Frame::end`] (input
//! samples taken when the frame completed, the chain's only time stamp), `span` is
//! [`common::span`] for the frame's range (its window at the input rate plus the filter's taps),
//! and `start`, `stop` are `window_ms` in samples, truncated as the reference truncates them. That
//! is the rule the reference applies to its own frames, so both statistics see the same audio.
//!
//! **What is measured,** per fixture, over those frames:
//! - the median estimate's error in cents against the reference (the upper median, `sorted[n/2]`,
//!   as the reference takes its own);
//! - *gross* frames: an estimate more than [`GROSS_CENTS`] from the reference, by frequency, so an
//!   octave error counts even when the frame is flagged `ambiguous octave`;
//! - *unvoiced* frames: no estimate at all (unclear, or withheld by the octave guard).
//!
//! **What is gated,** by the manifest's `set`:
//! - `gold`, recorded acoustic guitar: |median| within §1's ±2 ¢, no gross and no unvoiced frame;
//! - `ceiling`: |median| under its entry in [`CEILINGS`], a regression ceiling, not a budget
//!   (`T-003g` owns budgets for the instruments §1 does not name), and no unvoiced and no gross
//!   frame: a wrong note is no instrument's regression to allow. `bass_b0` is the octave guard on a
//!   real low B, M2's exit criterion; `ukulele_g4` a G4 over a tone at a third of it (`T-020`);
//! - `stress`: reported only.
//!
//! `cargo test -p diapason_dsp --test recorded -- --nocapture` prints the table.

use std::fs;
use std::path::{Path, PathBuf};

use diapason_dsp::chain::{Frame, Selection};
use diapason_dsp::convert::to_f32;
use diapason_dsp::octave::Octave;
use diapason_dsp::range::Range;
use serde_json::Value;

pub mod common;
use common::{Tuner, cents, report, span};

/// `docs/AUDIO_ENGINE.md` §1: recorded acoustic guitar fixtures, ±2 cents (fails at ±4).
const GOLD_BUDGET_CENTS: f64 = 2.0;
/// An estimate further than this from the reference names another note (half a semitone).
const GROSS_CENTS: f64 = 50.0;
/// The fixtures that hold the octave guard to a real low B (B0, 30.9 Hz) and to a high note over a
/// tone at a third of it (`T-020`): both must stay in the ceiling set, where no frame may be gross.
const OCTAVE_GUARDS: [&str; 2] = ["bass_b0", "ukulele_g4"];

/// **Not budgets.** Regression ceilings on |median error| in cents for the `ceiling` set, the
/// convention of `tests/accuracy.rs`: the value measured when the fixtures landed (`T-003e`, in the
/// comment beside each) rounded up to a whole cent, plus a cent. The chain is deterministic, so the
/// margin is for platforms' `f64` kernels and for small changes elsewhere in the chain, not for
/// noise. They catch a detector that gets worse; whether these instruments meet a budget, and
/// which, is `T-003g`'s to decide.
const CEILINGS: [(&str, f64); 14] = [
    ("bass_a1", 2.0),        // +0.188
    ("bass_b0", 2.0),        // −0.043
    ("bass_d2", 3.0),        // +1.545
    ("bass_e1", 2.0),        // −0.131
    ("electric_a2", 2.0),    // +0.496
    ("electric_a4", 2.0),    // −0.066
    ("electric_e2", 3.0),    // +1.265
    ("ukulele_a4", 2.0),     // −0.052
    ("ukulele_c4", 2.0),     // +0.259
    ("ukulele_e4", 3.0),     // +1.651
    ("ukulele_g4", 4.0),     // −2.577 since `T-020` (−2.583, with 20 gross frames, before)
    ("violin_pizz_a4", 2.0), // +0.432
    ("violin_pizz_e5", 2.0), // +0.162
    ("violin_pizz_g3", 3.0), // −1.025
];

/// What a fixture is held to: the manifest's `set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Set {
    Gold,
    Ceiling,
    Stress,
}

impl Set {
    fn name(self) -> &'static str {
        match self {
            Self::Gold => "gold",
            Self::Ceiling => "ceiling",
            Self::Stress => "stress",
        }
    }
}

fn range_name(range: Range) -> &'static str {
    match range {
        Range::Low => "Low",
        Range::Mid => "Mid",
        Range::High => "High",
    }
}

/// The subset of a manifest entry this test reads. The schema's home is
/// `rust/crates/xtask/src/fixtures/manifest.rs`.
#[derive(Debug)]
struct Fixture {
    id: String,
    file: String,
    instrument: String,
    set: Set,
    sample_rate: u32,
    bits: u16,
    reference_hz: f64,
    window_ms: [u64; 2],
}

/// One fixture's result: the row of the table, and what the gates read.
#[derive(Debug)]
struct Row {
    id: String,
    set: Set,
    home: Range,
    reference_hz: f64,
    /// Frames inside the window.
    frames: usize,
    /// Cents from the reference of the median estimate; NaN when there is none.
    median_cents: f64,
    /// The estimate furthest from the reference, in cents; NaN when there is none.
    worst_cents: f64,
    gross: usize,
    unvoiced: usize,
    ambiguous: usize,
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/audio")
}

fn text<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry[key]
        .as_str()
        .unwrap_or_else(|| panic!("manifest: `{key}` is not a string in {entry}"))
}

fn number(entry: &Value, key: &str) -> u64 {
    entry[key]
        .as_u64()
        .unwrap_or_else(|| panic!("manifest: `{key}` is not an integer in {entry}"))
}

/// Every fixture in the manifest, in its order (sorted by id).
fn manifest() -> Vec<Fixture> {
    let path = fixtures_dir().join("manifest.json");
    let json: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    json["fixtures"]
        .as_array()
        .expect("manifest: `fixtures` is an array")
        .iter()
        .map(|entry| {
            let reference = &entry["reference"];
            let window = reference["window_ms"]
                .as_array()
                .and_then(|w| Some([w.first()?.as_u64()?, w.get(1)?.as_u64()?]))
                .unwrap_or_else(|| panic!("manifest: no reference window in {entry}"));
            Fixture {
                id: text(entry, "id").to_owned(),
                file: text(entry, "file").to_owned(),
                instrument: text(entry, "instrument").to_owned(),
                set: match text(entry, "set") {
                    "gold" => Set::Gold,
                    "ceiling" => Set::Ceiling,
                    "stress" => Set::Stress,
                    other => panic!("manifest: unknown set `{other}`"),
                },
                sample_rate: u32::try_from(number(entry, "sample_rate")).unwrap(),
                bits: u16::try_from(number(entry, "bits")).unwrap(),
                reference_hz: reference["hz"]
                    .as_f64()
                    .unwrap_or_else(|| panic!("manifest: no reference pitch in {entry}")),
                window_ms: window,
            }
        })
        .collect()
}

/// The fixture's samples in [−1, 1): integer PCM over 2^(bits−1), as the reference reads them.
/// The header must agree with the manifest; `cargo xtask fixtures check` holds the bytes.
fn samples(fixture: &Fixture) -> Vec<f32> {
    let mut reader = hound::WavReader::open(fixtures_dir().join(&fixture.file)).unwrap();
    let spec = reader.spec();
    assert_eq!(
        (
            spec.channels,
            spec.sample_format,
            spec.sample_rate,
            spec.bits_per_sample
        ),
        (
            1,
            hound::SampleFormat::Int,
            fixture.sample_rate,
            fixture.bits
        ),
        "{}: the header disagrees with the manifest",
        fixture.id
    );
    let scale = f64::from(1_u32 << (fixture.bits - 1));
    reader
        .samples::<i32>()
        .map(|sample| to_f32(f64::from(sample.unwrap()) / scale))
        .collect()
}

/// The home range the instrument's preset would pick (`docs/AUDIO_ENGINE.md` §2's range table:
/// guitar E-standard Mid, the bass Low, violin and ukulele High). The presets themselves are M3's;
/// a new instrument in the manifest must be given one here.
fn home(instrument: &str) -> Range {
    match instrument {
        "acoustic-guitar" | "electric-guitar" => Range::Mid,
        "electric-bass" => Range::Low,
        "ukulele" | "violin" => Range::High,
        other => panic!("no home range for instrument `{other}`"),
    }
}

/// `ms` milliseconds in samples at `rate`, truncated, as `xtask fixtures reference` converts them.
fn at(ms: u64, rate: u32) -> u64 {
    ms * u64::from(rate) / 1_000
}

/// Stream `fixture` through the chain and measure it over its reference window.
fn measure(fixture: &Fixture) -> Row {
    let audio = samples(fixture);
    let home = home(&fixture.instrument);
    let mut tuner = Tuner::new(fixture.sample_rate, Selection::Auto(home));
    let spans = Range::ALL.map(|range| (range, span(tuner.table(), range)));
    let [start, stop] = fixture.window_ms.map(|ms| at(ms, fixture.sample_rate));
    // Frames after the window are not judged, so the audio after it is not analysed.
    let fed = usize::try_from(stop).unwrap().min(audio.len());
    let mut errors = Vec::new();
    let (mut frames, mut unvoiced, mut ambiguous) = (0, 0, 0);
    tuner.push(&audio[..fed], |frame: &Frame| {
        let span = spans
            .iter()
            .find_map(|&(range, span)| (range == frame.range).then_some(span))
            .unwrap();
        if frame.end > stop || frame.end < start + span {
            return;
        }
        frames += 1;
        match frame.estimate {
            Some(estimate) => {
                errors.extend([cents(estimate.frequency_hz, fixture.reference_hz)]);
                if frame.octave == Octave::Ambiguous {
                    ambiguous += 1;
                }
            }
            None => unvoiced += 1,
        }
    });
    errors.sort_by(f64::total_cmp);
    Row {
        id: fixture.id.clone(),
        set: fixture.set,
        home,
        reference_hz: fixture.reference_hz,
        frames,
        median_cents: errors.get(errors.len() / 2).copied().unwrap_or(f64::NAN),
        worst_cents: errors
            .iter()
            .copied()
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(f64::NAN),
        gross: errors.iter().filter(|e| e.abs() > GROSS_CENTS).count(),
        unvoiced,
        ambiguous,
    }
}

/// Measure every fixture of `set`, a thread each, and report the table.
fn measure_set(set: Set) -> Vec<Row> {
    let fixtures: Vec<Fixture> = manifest().into_iter().filter(|f| f.set == set).collect();
    assert!(!fixtures.is_empty(), "no {set:?} fixtures in the manifest");
    let rows: Vec<Row> = std::thread::scope(|scope| {
        let handles: Vec<_> = fixtures
            .iter()
            .map(|fixture| scope.spawn(move || measure(fixture)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    report(format_args!(
        "{:<20} {:<8} {:<5} {:>10} {:>7} {:>10} {:>10} {:>6} {:>9} {:>10}",
        "fixture",
        "set",
        "home",
        "ref Hz",
        "frames",
        "median ¢",
        "worst ¢",
        "gross",
        "unvoiced",
        "ambiguous"
    ));
    for row in &rows {
        report(format_args!(
            "{:<20} {:<8} {:<5} {:>10.4} {:>7} {:>+10.3} {:>+10.1} {:>6} {:>9} {:>10}",
            row.id,
            row.set.name(),
            range_name(row.home),
            row.reference_hz,
            row.frames,
            row.median_cents,
            row.worst_cents,
            row.gross,
            row.unvoiced,
            row.ambiguous
        ));
    }
    rows
}

/// Whether `cents` is within ±`limit`; a NaN (no estimate) never is.
fn within(cents: f64, limit: f64) -> bool {
    cents.abs() <= limit
}

fn ceiling(id: &str) -> Option<f64> {
    CEILINGS
        .iter()
        .find_map(|&(name, ceiling)| (name == id).then_some(ceiling))
}

#[test]
fn every_ceiling_fixture_has_exactly_one_ceiling_and_the_octave_guards_are_there() {
    let fixtures = manifest();
    let ceilings: Vec<&str> = fixtures
        .iter()
        .filter(|f| f.set == Set::Ceiling)
        .map(|f| f.id.as_str())
        .collect();
    let mut named: Vec<&str> = CEILINGS.iter().map(|&(id, _)| id).collect();
    named.sort_unstable();
    named.dedup();
    assert_eq!(named.len(), CEILINGS.len(), "a fixture has two ceilings");
    assert_eq!(
        named, ceilings,
        "CEILINGS and the manifest's ceiling set differ"
    );
    for id in OCTAVE_GUARDS {
        assert!(ceilings.contains(&id), "{id} is not a ceiling fixture");
    }
    assert!(fixtures.iter().any(|f| f.set == Set::Gold));
}

/// §1's budget, gated: every recorded acoustic guitar note within ±2 ¢ over its window, with no
/// frame on another note and none withheld: a guard that withholds every frame must not pass.
#[test]
fn recorded_acoustic_guitar_is_within_two_cents_and_never_on_another_note() {
    let rows = measure_set(Set::Gold);
    let failing: Vec<&Row> = rows
        .iter()
        .filter(|row| {
            !within(row.median_cents, GOLD_BUDGET_CENTS)
                || row.gross > 0
                || row.unvoiced > 0
                || row.frames == 0
        })
        .collect();
    assert!(
        failing.is_empty(),
        "over §1's ±{GOLD_BUDGET_CENTS} ¢, or with a gross or unvoiced frame: {failing:#?}"
    );
}

/// **Not the budget.** The other instruments under their regression ceilings, never on another
/// note. None withholds a frame today, so none may start to.
#[test]
fn other_instruments_stay_under_their_ceilings_and_never_show_another_note() {
    let rows = measure_set(Set::Ceiling);
    let failing: Vec<(&Row, Option<f64>)> = rows
        .iter()
        .map(|row| (row, ceiling(&row.id)))
        .filter(|&(row, limit)| {
            !limit.is_some_and(|limit| within(row.median_cents, limit))
                || row.frames == 0
                || row.unvoiced > 0
                || row.gross > 0
        })
        .collect();
    assert!(
        failing.is_empty(),
        "over a ceiling, or with an unvoiced or a gross frame: {failing:#?}"
    );
}

/// Reported, never gated: notes whose pitch will not hold still (vibrato).
#[test]
fn stress_fixtures_are_recorded() {
    let rows = measure_set(Set::Stress);
    assert!(rows.iter().all(|row| row.frames > 0));
}

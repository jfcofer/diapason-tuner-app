//! Replays a WAV through the shipped chain and prints every frame (`T-026`).
//!
//! `cargo run -p diapason_dsp --release --example replay -- <file.wav> [low|mid|high] [expected-note]`,
//! or `just replay`. It uses `tests/common`'s `Tuner`, the glue `tests/recorded.rs` uses, so a take
//! kept outside the repo is judged by the same chain as a checked-in fixture.
//!
//! Stdout is a TSV of frames, then a `#` summary. The expected note is a MIDI number, for a single
//! string: `wrong` counts frames naming any other note, and `other octave` those naming its pitch
//! class elsewhere. The two partial levels compare a take's fundamental with its second partial. They
//! describe the sound the chain was given, and cannot say whether a phone, a room or a guitar made it.

use std::f64::consts::TAU;
use std::io::{self, Write};
use std::process::ExitCode;

use diapason_dsp::chain::Selection;
use diapason_dsp::note::nearest;
use diapason_dsp::octave::Octave;
use diapason_dsp::range::{Range, RangeTable};

#[path = "../tests/common/mod.rs"]
pub mod common;
use common::Tuner;

const A4: f64 = 440.0;
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// One frame's reading, kept for the summary.
#[derive(Clone, Copy)]
struct Reading {
    note: i32,
    cents: f64,
    hz: f64,
}

/// A note number's name, written straight to `out`: `format!` is on `dsp`'s deny list.
fn write_note(out: &mut impl Write, note: i32) -> io::Result<()> {
    let pitch = usize::try_from(note.rem_euclid(12)).unwrap_or(0);
    write!(out, "{}{}", NAMES[pitch], note.div_euclid(12) - 1)
}

fn range_name(range: Range) -> &'static str {
    match range {
        Range::Low => "low",
        Range::Mid => "mid",
        Range::High => "high",
    }
}

/// How far from a partial's nominal frequency to look for it. A guitar's partials are not whole
/// multiples: steel E2's sit up to 7.7 ¢ sharp of its first (`docs/adr/0030`), a large miss for a
/// long window, so a level read at exactly `2 × f₀` would measure the window's skirt.
const SEARCH: f64 = 0.03;
/// Half a second: short enough to follow a decaying partial, and it separates E2's first two.
const LEVEL_FRAME_SECONDS: f64 = 0.5;

/// The level of the partial near `hz` over `samples`, in dB of amplitude: in each half-overlapping
/// Hann frame, the highest Goertzel magnitude within [`SEARCH`] of `hz`, and the mean of their powers.
fn level_db(samples: &[f32], rate: u32, hz: f64) -> f64 {
    let rate_hz = f64::from(rate);
    let length = diapason_dsp::convert::index_from_f64(LEVEL_FRAME_SECONDS * rate_hz);
    let size = diapason_dsp::convert::index_to_f64(length);
    let hann: Vec<f64> = (0..length)
        .map(|i| 0.5 - 0.5 * (TAU * diapason_dsp::convert::index_to_f64(i) / size).cos())
        .collect();
    let weight_sum: f64 = hann.iter().sum();
    let step = rate_hz / size / 8.0;
    let trials = diapason_dsp::convert::index_from_f64(2.0 * SEARCH * hz / step) + 1;
    let (mut total, mut frames) = (0.0_f64, 0.0_f64);
    for frame in samples.windows(length).step_by(length / 2) {
        let mut peak = 0.0_f64;
        for trial in 0..trials {
            let at = hz * (1.0 - SEARCH) + step * diapason_dsp::convert::index_to_f64(trial);
            let coefficient = 2.0 * (TAU * at / rate_hz).cos();
            let (mut s1, mut s2) = (0.0_f64, 0.0_f64);
            for (&sample, &weight) in frame.iter().zip(&hann) {
                let s0 = f64::from(sample) * weight + coefficient * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            peak = peak.max(s1 * s1 + s2 * s2 - coefficient * s1 * s2);
        }
        total += peak;
        frames += 1.0;
    }
    let amplitude = 2.0 * (total / frames.max(1.0)).sqrt() / weight_sum;
    20.0 * amplitude.max(1e-12).log10()
}

fn read(path: &str) -> Result<(Vec<f32>, u32), hound::Error> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_format != hound::SampleFormat::Int {
        return Err(hound::Error::Unsupported);
    }
    let scale = f64::from(1_u32 << (spec.bits_per_sample - 1));
    let samples = reader
        .samples::<i32>()
        .map(|sample| sample.map(|s| diapason_dsp::convert::to_f32(f64::from(s) / scale)))
        .collect::<Result<Vec<f32>, _>>()?;
    Ok((samples, spec.sample_rate))
}

fn replay(path: &str, home: Range, expected: Option<i32>, out: &mut impl Write) -> io::Result<()> {
    let (samples, rate) = read(path).map_err(io::Error::other)?;
    if RangeTable::new(rate).is_err() {
        return Err(io::Error::other(
            "the chain does not support this sample rate",
        ));
    }
    let mut tuner = Tuner::new(rate, Selection::Auto(home));
    let (mut frames, mut unvoiced, mut held, mut ambiguous) = (0_u32, 0_u32, 0_u32, 0_u32);
    let (mut first, mut last) = (None, 0_u64);
    let mut readings: Vec<Reading> = Vec::with_capacity(samples.len() / 64);
    let mut failure = Ok(());
    writeln!(
        out,
        "ms\trange\thz\tnote\tcents\tclarity\toctave\theld\tneedle_hz"
    )?;
    tuner.push(&samples, |frame| {
        frames += 1;
        held += u32::from(frame.held);
        ambiguous += u32::from(frame.octave == Octave::Ambiguous);
        let ms = diapason_dsp::convert::index_to_f64(usize::try_from(frame.end).unwrap_or(0))
            * 1_000.0
            / f64::from(rate);
        let row = (|| -> io::Result<()> {
            write!(out, "{ms:.1}\t{}\t", range_name(frame.range))?;
            if let Some(estimate) = frame.estimate {
                let hz = estimate.frequency_hz;
                first.get_or_insert(frame.end);
                last = frame.end;
                let named = nearest(hz, A4, 0);
                write!(out, "{hz:.3}\t")?;
                if let Some(named) = named {
                    write_note(out, named.note)?;
                    write!(out, "\t{:+.2}\t", named.cents)?;
                    // Not `push`, which `dsp`'s real-time list bans. This tool is never on that path.
                    readings.extend(std::iter::once(Reading {
                        note: named.note,
                        cents: named.cents,
                        hz,
                    }));
                } else {
                    write!(out, "-\t-\t")?;
                }
                write!(out, "{:.3}\t", estimate.clarity)?;
            } else {
                unvoiced += 1;
                write!(out, "-\t-\t-\t-\t")?;
            }
            let octave = if frame.octave == Octave::Known {
                "known"
            } else {
                "ambiguous"
            };
            write!(out, "{octave}\t{}\t", u8::from(frame.held))?;
            match frame.display {
                Some(hz) => writeln!(out, "{hz:.3}"),
                None => writeln!(out, "-"),
            }
        })();
        if failure.is_ok() {
            failure = row;
        }
    });
    failure?;

    let seconds = diapason_dsp::convert::index_to_f64(samples.len()) / f64::from(rate);
    writeln!(
        out,
        "# {path}: {seconds:.1} s at {rate} Hz, home {}",
        range_name(home)
    )?;
    writeln!(
        out,
        "# frames {frames}, with an estimate {}, unvoiced {unvoiced}, held {held}, ambiguous octave {ambiguous}",
        readings.len()
    )?;

    // The most-read note, and each note's share.
    let mut notes: Vec<i32> = readings.iter().map(|r| r.note).collect();
    notes.sort_unstable();
    notes.dedup();
    let mut shares: Vec<(i32, usize)> = notes
        .iter()
        .map(|&n| (n, readings.iter().filter(|r| r.note == n).count()))
        .collect();
    shares.sort_unstable_by_key(|&(_, count)| std::cmp::Reverse(count));
    write!(out, "# notes:")?;
    for &(note, count) in shares.iter().take(8) {
        write!(out, " ")?;
        write_note(out, note)?;
        write!(out, "×{count}")?;
    }
    writeln!(out)?;

    let target = expected.or_else(|| shares.first().map(|&(note, _)| note));
    if let Some(target) = target {
        let mut cents: Vec<f64> = readings
            .iter()
            .filter(|r| r.note == target)
            .map(|r| r.cents)
            .collect();
        cents.sort_unstable_by(f64::total_cmp);
        let mut hz: Vec<f64> = readings
            .iter()
            .filter(|r| r.note == target)
            .map(|r| r.hz)
            .collect();
        hz.sort_unstable_by(f64::total_cmp);
        write!(out, "# target ")?;
        write_note(out, target)?;
        if let (Some(&median), Some(&f0)) = (cents.get(cents.len() / 2), hz.get(hz.len() / 2)) {
            writeln!(
                out,
                ": median {median:+.2} ¢ at {f0:.3} Hz over {} frames",
                cents.len()
            )?;
            // The partials over the voiced span, after the attack.
            let to_index = |at: u64| usize::try_from(at).unwrap_or(0).min(samples.len());
            let start = to_index(first.unwrap_or(0));
            let voiced = samples.get(start..to_index(last)).unwrap_or(&[]);
            if voiced.len() > 24_000 {
                let (f1, f2) = (level_db(voiced, rate, f0), level_db(voiced, rate, 2.0 * f0));
                writeln!(
                    out,
                    "# partials over the voiced span: first {f1:.1} dB, second {f2:.1} dB ({:+.1} dB)",
                    f1 - f2
                )?;
            }
        } else {
            writeln!(out, ": never read")?;
        }
        if expected.is_some() {
            let wrong = readings.iter().filter(|r| r.note != target).count();
            let octaves = readings
                .iter()
                .filter(|r| r.note != target && (r.note - target).rem_euclid(12) == 0)
                .count();
            writeln!(
                out,
                "# wrong frames {wrong} of {} (other octave {octaves})",
                readings.len()
            )?;
        }
    } else {
        writeln!(out, "# no note was read")?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = match args.get(1).map(String::as_str) {
        None | Some("mid") => Some(Range::Mid),
        Some("low") => Some(Range::Low),
        Some("high") => Some(Range::High),
        Some(_) => None,
    };
    let expected = args.get(2).map(|note| note.parse::<i32>());
    let (Some(path), Some(home), None | Some(Ok(_))) = (args.first(), home, expected.clone())
    else {
        let usage = "usage: replay <file.wav> [low|mid|high] [expected MIDI note]\n";
        return match io::stderr().write_all(usage.as_bytes()) {
            Ok(()) | Err(_) => ExitCode::FAILURE,
        };
    };
    let expected = expected.and_then(Result::ok);
    let mut out = io::stdout().lock();
    match replay(path, home, expected, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            let _ = writeln!(io::stderr(), "replay: {path}: {reason}");
            ExitCode::FAILURE
        }
    }
}

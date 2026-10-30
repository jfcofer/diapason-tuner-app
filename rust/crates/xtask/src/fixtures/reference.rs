//! `cargo xtask fixtures reference`: measure a fixture's pitch by a method that shares nothing with
//! the detector under test (`docs/adr/0030`).
//!
//! The detector (`diapason_dsp::pitch`) reads periodicity in the time domain. This reads each
//! partial's frequency from the phase it advances by between frames: demodulate at a trial
//! frequency, window, and the phase step over a known hop is the frequency offset. The truth is the
//! harmonic consensus, the amplitude-weighted mean of `f_k / k` over the first six partials (owner,
//! 2026-10-30). On recorded guitars the upper partials sit up to 9 cents from the first, which the
//! body pulls off the series: up to 3.6 cents from the consensus, and 4.5 on a bass. The first
//! partial is recorded beside it.

use std::f64::consts::TAU;
use std::path::Path;

use diapason_dsp::convert::{index_from_f64, index_to_f64};

use super::Args;
use super::manifest::{self, Reference};

/// Periods of the nominal pitch in one analysis frame. Twelve keep the Hann main lobe (± two bins)
/// well inside the gap to the second partial, and frames short enough to follow a decaying pluck.
const PERIODS_PER_FRAME: f64 = 12.0;
/// Frames advance by an eighth of their length: the phase step stays unambiguous for any offset
/// the coarse search leaves.
const HOP_DIVISOR: usize = 8;
/// The coarse search spans a semitone either side of the nominal note, in one-cent steps. A peak at
/// either edge means the note is mislabelled, and the measurement is refused.
const SEARCH_CENTS: i32 = 100;
/// An octave check, as a power ratio (−30 dB). The partial found must not be dwarfed by one an
/// octave up (the label is an octave low), and nothing an octave down may reach it (the label is an
/// octave high: Karoryfer's bass names). A low string's fundamental can sit 20 dB under its second
/// partial, so the margin is wide.
const OCTAVE_RATIO: f64 = 1e-3;
/// The partials the consensus reads (owner, 2026-10-30): for a low note the ear's pitch comes from
/// the second to fifth, not the first, and a guitar's body pulls the first off the series.
const PARTIALS: usize = 6;
/// Each partial is found within this many cents of `k` times the first.
const PARTIAL_SEARCH_CENTS: i32 = 30;
/// A partial under the strongest by more than this power ratio (−30 dB) is noise, and left out.
const FLOOR_RATIO: f64 = 1e-3;
/// Partials are read below this fraction of the sample rate, clear of the anti-aliasing filter.
const NYQUIST_FRACTION: f64 = 0.45;

/// Run `reference` for `--id`, over `--window-ms a,b` (default 300,1300).
pub fn run(args: &Args) -> Result<(), String> {
    let root = manifest::dir();
    let id = args.get("id")?;
    let window = match args.optional("window-ms") {
        None => [300, 1300],
        Some(text) => parse_window(text)?,
    };
    let mut manifest = manifest::load(&root)?;
    let fixture = manifest
        .fixtures
        .iter_mut()
        .find(|f| f.id == id)
        .ok_or_else(|| format!("no fixture `{id}`"))?;
    let (samples, rate) = read_mono(&root.join(&fixture.file))?;
    let nominal = note_hz(&fixture.note)?;
    let reference = measure(&samples, rate, nominal, window)?;
    println!(
        "{id}: {:.4} Hz ({:+.2} cents from {}), uncertainty {:.3} cents, glide {:+.2} cents",
        reference.hz,
        cents(reference.hz, nominal),
        fixture.note,
        reference.uncertainty_cents,
        reference.glide_cents
    );
    fixture.reference = Some(reference);
    manifest::save(&root, &mut manifest)
}

/// The harmonic consensus near `nominal` over `window_ms` (`docs/adr/0030`), by phase advance.
pub fn measure(
    samples: &[f64],
    rate: u32,
    nominal: f64,
    window_ms: [u32; 2],
) -> Result<Reference, String> {
    let fs = f64::from(rate);
    let to_frame =
        |ms: u32| usize::try_from(u64::from(ms) * u64::from(rate) / 1000).unwrap_or(usize::MAX);
    let (start, end) = (
        to_frame(window_ms[0]),
        to_frame(window_ms[1]).min(samples.len()),
    );
    let n = index_from_f64(PERIODS_PER_FRAME * fs / nominal);
    let hop = (n / HOP_DIVISOR).max(1);
    if end <= start || end - start < 2 * n {
        return Err(format!(
            "window {window_ms:?} ms is too short for two {n}-sample frames"
        ));
    }
    let hann: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * (TAU * index_to_f64(i) / index_to_f64(n)).cos())
        .collect();

    // Coarse: the strongest trial frequency over the whole window, in a Hann taper of its length.
    let span = &samples[start..end];
    let taper: Vec<f64> = (0..span.len())
        .map(|i| 0.5 - 0.5 * (TAU * index_to_f64(i) / index_to_f64(span.len())).cos())
        .collect();
    let (best_cents, best_power) = strongest(span, &taper, nominal / fs, SEARCH_CENTS);
    if best_power <= f64::MIN_POSITIVE {
        return Err(format!("the window {window_ms:?} ms is silent"));
    }
    if best_cents.abs() == SEARCH_CENTS {
        return Err(format!(
            "the strongest partial is a semitone or more from {nominal:.3} Hz: check the note"
        ));
    }
    if best_power < OCTAVE_RATIO * strongest(span, &taper, 2.0 * nominal / fs, 50).1 {
        return Err(format!(
            "the partial an octave above {nominal:.3} Hz dominates: is the note an octave low?"
        ));
    }
    if strongest(span, &taper, nominal / 2.0 / fs, 50).1 > OCTAVE_RATIO * best_power {
        return Err(format!(
            "there is a partial an octave below {nominal:.3} Hz: is the note an octave high?"
        ));
    }
    let first = nominal * (f64::from(best_cents) / 1200.0).exp2();

    let partials: Vec<_> = find_partials(span, &taper, first, fs)
        .into_iter()
        .map(|(k, amplitude, trial)| {
            (
                k,
                amplitude,
                track(samples, &hann, trial, fs, start, end, hop),
            )
        })
        .collect();
    if partials.first().is_none_or(|p| p.0 != 1) {
        return Err("no first partial above the floor: silence, or not this note".into());
    }
    let used: Vec<String> = partials.iter().map(|p| p.0.to_string()).collect();

    // Per frame: the amplitude-weighted mean of f_k / k, in cents from the nominal note.
    let total: f64 = partials.iter().map(|p| p.1).sum();
    let frames = partials.iter().map(|p| p.2.len()).min().unwrap_or(0);
    let points: Vec<(f64, f64, f64)> = (0..frames)
        .map(|i| {
            let c = partials
                .iter()
                .map(|(k, w, track)| w * cents(track[i].1 / index_to_f64(*k), nominal))
                .sum::<f64>()
                / total;
            (partials[0].2[i].0, c, nominal * (c / 1200.0).exp2())
        })
        .collect();
    if points.len() < 2 {
        return Err("too few frames in the window".into());
    }

    let median_of = |mut hz: Vec<f64>| {
        hz.sort_by(f64::total_cmp);
        hz[hz.len() / 2]
    };
    let median = median_of(points.iter().map(|p| p.2).collect());
    let first_partial = median_of(partials[0].2.iter().map(|p| p.1).collect());
    let (slope, residual) = linear_fit(&points);
    let independent = index_to_f64(end - start) / index_to_f64(n);
    Ok(Reference {
        hz: round_to(median, 4),
        first_partial_hz: round_to(first_partial, 4),
        window_ms,
        method: format!(
            "harmonic consensus: amplitude-weighted f_k/k over partials {}, each by phase advance; \
             Hann {n} samples ({PERIODS_PER_FRAME} periods), hop {hop}, median",
            used.join(",")
        ),
        // Standard error of a median, from frames that overlap by HOP_DIVISOR, so counted as the
        // number of non-overlapping ones.
        uncertainty_cents: round_to(1.2533 * residual / independent.sqrt(), 3),
        glide_cents: round_to(slope * index_to_f64(end - start) / fs, 2),
    })
}

/// Each partial up to [`PARTIALS`] that stands above the floor: its number, its amplitude over the
/// window, and its frequency, found near `k` times the first.
fn find_partials(span: &[f64], taper: &[f64], first: f64, fs: f64) -> Vec<(usize, f64, f64)> {
    let candidates: Vec<(usize, f64, i32, f64)> = (1..=PARTIALS)
        .map(|k| (k, index_to_f64(k) * first))
        .take_while(|&(_, near)| near < NYQUIST_FRACTION * fs)
        .map(|(k, near)| {
            let (offset, power) = strongest(span, taper, near / fs, PARTIAL_SEARCH_CENTS);
            (k, near, offset, power)
        })
        .collect();
    let loudest = candidates.iter().map(|c| c.3).fold(0.0, f64::max);
    candidates
        .into_iter()
        .filter(|&(_, _, offset, power)| {
            offset.abs() < PARTIAL_SEARCH_CENTS && power >= FLOOR_RATIO * loudest
        })
        .map(|(k, near, offset, power)| {
            (k, power.sqrt(), near * (f64::from(offset) / 1200.0).exp2())
        })
        .collect()
}

/// One partial's frequency per frame, `(time, Hz)`: the phase each frame advances over one hop, at
/// the trial frequency, is the offset from it.
fn track(
    samples: &[f64],
    hann: &[f64],
    trial: f64,
    fs: f64,
    start: usize,
    end: usize,
    hop: usize,
) -> Vec<(f64, f64)> {
    let n = hann.len();
    let mut previous = None;
    let mut out = Vec::new();
    let mut at = start;
    while at + n <= end {
        let z = demodulate(&samples[at..at + n], hann, trial / fs, at);
        if let Some(p) = previous {
            let step = (z * conj(p)).arg();
            let t = (index_to_f64(at) + index_to_f64(n) / 2.0) / fs;
            out.push((t, trial + step * fs / (TAU * index_to_f64(hop))));
        }
        previous = Some(z);
        at += hop;
    }
    out
}

/// The offset in cents, within ±`span_cents` of `f` (cycles per sample), with the most power, and
/// that power.
fn strongest(x: &[f64], window: &[f64], f: f64, span_cents: i32) -> (i32, f64) {
    (-span_cents..=span_cents)
        .map(|c| {
            (
                c,
                demodulate(x, window, f * (f64::from(c) / 1200.0).exp2(), 0).norm_sqr(),
            )
        })
        .fold(
            (0, 0.0),
            |best, next| if next.1 > best.1 { next } else { best },
        )
}

/// `Σ w[i]·x[i]·e^(−i·2π·f·(offset + i))`, with `f` in cycles per sample. Phase is measured from the
/// start of the file, so consecutive frames compare directly.
///
/// The oscillator is a phasor turned by a fixed step, not a `sin` and `cos` per sample: a tenth of
/// the cost, and re-anchored every [`REANCHOR`] samples so rounding cannot build up.
fn demodulate(x: &[f64], window: &[f64], f: f64, offset: usize) -> Complex {
    let at = |i: usize| {
        let phase = -TAU * f * index_to_f64(offset + i);
        Complex(phase.cos(), phase.sin())
    };
    let step = Complex((-TAU * f).cos(), (-TAU * f).sin());
    let mut z = Complex(0.0, 0.0);
    let mut osc = at(0);
    for (i, (&s, &w)) in x.iter().zip(window).enumerate() {
        if i % REANCHOR == 0 {
            osc = at(i);
        }
        z.0 += s * w * osc.0;
        z.1 += s * w * osc.1;
        osc = osc * step;
    }
    z
}

/// Samples between exact recomputations of [`demodulate`]'s oscillator. Each turn rounds by about
/// 1e-16, so the phase stays within 1e-13 radians of exact.
const REANCHOR: usize = 1024;

/// Least-squares slope of cents against seconds, and the residuals' standard deviation.
fn linear_fit(points: &[(f64, f64, f64)]) -> (f64, f64) {
    let count = index_to_f64(points.len());
    let mean_t = points.iter().map(|p| p.0).sum::<f64>() / count;
    let mean_c = points.iter().map(|p| p.1).sum::<f64>() / count;
    let sxx: f64 = points.iter().map(|p| (p.0 - mean_t).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mean_t) * (p.1 - mean_c)).sum();
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let residual = points
        .iter()
        .map(|p| (p.1 - mean_c - slope * (p.0 - mean_t)).powi(2))
        .sum::<f64>()
        / (count - 2.0).max(1.0);
    (slope, residual.sqrt())
}

/// A WAV's samples as `f64` in [−1, 1).
pub fn read_mono(path: &Path) -> Result<(Vec<f64>, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_format != hound::SampleFormat::Int {
        return Err(format!("{}: fixtures are mono integer PCM", path.display()));
    }
    let scale = f64::from(1_u32 << (spec.bits_per_sample - 1));
    let samples = reader
        .samples::<i32>()
        .map(|s| s.map(|v| f64::from(v) / scale))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok((samples, spec.sample_rate))
}

/// A note in scientific pitch notation (`A4`, `C#5`, `Bb3`) at A4 = 440 Hz.
pub fn note_hz(note: &str) -> Result<f64, String> {
    let bad = || format!("note `{note}`: expected a letter, an optional # or b, and an octave");
    let mut chars = note.chars();
    let letter = match chars.next().ok_or_else(bad)? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return Err(bad()),
    };
    let rest = chars.as_str();
    let (shift, octave) = match rest.strip_prefix('#') {
        Some(o) => (1, o),
        None => rest.strip_prefix('b').map_or((0, rest), |o| (-1, o)),
    };
    let octave: i32 = octave.parse().map_err(|_| bad())?;
    let midi = 12 * (octave + 1) + letter + shift;
    Ok(440.0 * (f64::from(midi - 69) / 12.0).exp2())
}

fn parse_window(text: &str) -> Result<[u32; 2], String> {
    let parts: Vec<u32> = text
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| format!("--window-ms {text}: expected two integers, start,end"))?;
    match parts[..] {
        [a, b] if b > a => Ok([a, b]),
        _ => Err(format!(
            "--window-ms {text}: expected start,end with end after start"
        )),
    }
}

pub fn cents(f: f64, reference: f64) -> f64 {
    1200.0 * (f / reference).log2()
}

/// Rounded to `places` decimals. Dividing by the power of ten, rather than multiplying by a step,
/// gives the double nearest the decimal, so the manifest reads `30.8663`, not `30.866300000000003`.
fn round_to(value: f64, places: i32) -> f64 {
    let scale = 10_f64.powi(places);
    (value * scale).round() / scale
}

/// Just enough complex arithmetic for one demodulator.
#[derive(Clone, Copy)]
struct Complex(f64, f64);

impl Complex {
    fn norm_sqr(self) -> f64 {
        self.0 * self.0 + self.1 * self.1
    }
    fn arg(self) -> f64 {
        self.1.atan2(self.0)
    }
}

impl std::ops::Mul for Complex {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
}

fn conj(z: Complex) -> Complex {
    Complex(z.0, -z.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diapason_dsp::synth::{self, Pluck};

    #[test]
    fn notes_parse_at_a440() {
        assert!((note_hz("A4").unwrap() - 440.0).abs() < 1e-9);
        assert!((note_hz("B0").unwrap() - 30.867_706).abs() < 1e-5);
        assert!((note_hz("C#5").unwrap() - note_hz("Db5").unwrap()).abs() < 1e-9);
        assert!(note_hz("H2").is_err());
    }

    /// A decaying harmonic tone 7 cents sharp with a strong second partial: the consensus is its
    /// pitch, to a thousandth of a cent.
    #[test]
    fn a_decaying_two_partial_tone_is_measured_to_a_thousandth_of_a_cent() {
        let rate = 48_000;
        let f = 82.406_9 * (7.0_f64 / 1200.0).exp2();
        let samples: Vec<f64> = (0..rate * 2)
            .map(|i| {
                let t = f64::from(i) / f64::from(rate);
                (-t / 1.5).exp() * ((TAU * f * t).sin() + 0.8 * (TAU * 2.0 * f * t + 0.3).sin())
            })
            .collect();
        let r = measure(&samples, 48_000, 82.406_9, [300, 1300]).unwrap();
        assert!(cents(r.hz, f).abs() < 1e-3, "{} Hz against {f}", r.hz);
        assert!(r.uncertainty_cents < 0.01);
    }

    /// Two equal partials, the second 8 cents sharp of the series: the consensus sits halfway, and
    /// the first partial is recorded where it is.
    #[test]
    fn the_consensus_weighs_each_partial_by_its_amplitude() {
        let f = 110.0;
        let samples: Vec<f64> = (0..96_000)
            .map(|i| {
                let t = f64::from(i) / 48_000.0;
                (TAU * f * t).sin() + (TAU * 2.0 * f * (8.0_f64 / 1200.0).exp2() * t).sin()
            })
            .collect();
        let r = measure(&samples, 48_000, f, [300, 1300]).unwrap();
        assert!(
            (cents(r.hz, f) - 4.0).abs() < 0.01,
            "{} cents",
            cents(r.hz, f)
        );
        assert!(cents(r.first_partial_hz, f).abs() < 0.01);
    }

    /// The inharmonic pluck `dsp/tests/accuracy.rs` uses: its partials stretch sharp, so the
    /// consensus sits above the first partial and below the sixth's `f_6 / 6`.
    #[test]
    fn an_inharmonic_pluck_reads_between_its_first_and_sixth_partials() {
        let params = Pluck::default();
        let f0 = 110.0;
        let f1 = synth::partial_frequency(f0, 1, params.inharmonicity);
        let f6 = synth::partial_frequency(f0, 6, params.inharmonicity) / 6.0;
        let mut signal = vec![0.0_f32; 96_000];
        synth::pluck(&mut signal, f0, 48_000, 0.8, &params);
        let samples: Vec<f64> = signal.iter().map(|&s| f64::from(s)).collect();
        let r = measure(&samples, 48_000, 110.0, [300, 1300]).unwrap();
        assert!(
            cents(r.first_partial_hz, f1).abs() < 0.05,
            "first partial {}",
            r.first_partial_hz
        );
        assert!(r.hz > f1 && r.hz < f6, "{} Hz, outside ({f1}, {f6})", r.hz);
    }

    #[test]
    fn silence_is_refused() {
        assert!(measure(&vec![0.0; 96_000], 48_000, 110.0, [300, 1300]).is_err());
    }

    /// A third partial 40 dB under the first is noise, and the consensus leaves it out.
    #[test]
    fn a_partial_under_the_floor_is_left_out() {
        let f = 110.0;
        let samples: Vec<f64> = (0..96_000)
            .map(|i| {
                let t = f64::from(i) / 48_000.0;
                (TAU * f * t).sin() + 0.01 * (TAU * 3.0 * f * (20.0_f64 / 1200.0).exp2() * t).sin()
            })
            .collect();
        let r = measure(&samples, 48_000, f, [300, 1300]).unwrap();
        assert!(r.method.contains("partials 1,"), "{}", r.method);
        assert!(!r.method.contains(",3"), "{}", r.method);
        assert!(cents(r.hz, f).abs() < 0.01);
    }

    #[test]
    fn a_mislabelled_octave_is_refused() {
        // Two partials, 110 and 220 Hz: labelled A3 it is an octave high, labelled A1 an octave low.
        let samples: Vec<f64> = (0..96_000)
            .map(|i| {
                let t = f64::from(i) / 48_000.0;
                (TAU * 110.0 * t).sin() + 0.5 * (TAU * 220.0 * t).sin()
            })
            .collect();
        assert!(measure(&samples, 48_000, 110.0, [300, 1300]).is_ok());
        assert!(
            measure(&samples, 48_000, 220.0, [300, 1300]).is_err(),
            "an octave high"
        );
        assert!(
            measure(&samples, 48_000, 55.0, [300, 1300]).is_err(),
            "an octave low"
        );
    }
}

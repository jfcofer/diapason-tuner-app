//! The tuner's analysis chain, end to end, on synthetic fixtures (`docs/AUDIO_ENGINE.md` §1–§2,
//! `docs/adr/0028-windows-hops-from-the-latency-ledger.md`, `docs/TESTING.md` §2).
//!
//! Every signal is *streamed* through the shipped chain (`diapason_dsp::chain::Front` and
//! `Analysis`, glued as the engine will glue them in `common::Tuner`), block by block: DC block,
//! gate, band-limit or ×4 decimation, frames, `Mpm`, selector. Nothing hands a frame to the
//! detector directly. Each test runs at every rate in `SUPPORTED_RATES`.
//!
//! - The filters meet their specification: exactly, from their taps, and measured with sines.
//! - The decimated path's accuracy, probed and recorded: where the crossover can sit.
//! - **The headline gate:** sine and sawtooth within ±0.5 ¢ over 30–1400 Hz, 24 points per
//!   octave plus every whole-period frequency of each window ±0.3 Hz, 8 phases, in every range
//!   that can hold each frequency.
//! - Onset latency, per range, within its line of the ledger.
//! - The selector: from home it finds each note's range, and it does not stay on the octave above
//!   a low note.
//! - In a room louder than the gate's minimum threshold, the gate learns the room and closes;
//!   a held note still keeps it open.
//!
//! `cargo test -p diapason_dsp --test chain -- --nocapture` prints the measured tables.

use std::f64::consts::TAU;
use std::fmt;
use std::io::Write;

use diapason_dsp::chain::{Selection, UNPITCHED_SECONDS};
use diapason_dsp::convert::{index_from_f64, index_to_f64};
use diapason_dsp::filter::{DcBlock, Decimator};
use diapason_dsp::pitch::{Mpm, MpmConfig};
use diapason_dsp::range::{
    CROSSOVER_HZ, DECIMATION, HYSTERESIS, Path, Range, RangeTable, SUPPORTED_RATES,
};
use diapason_dsp::synth::{self, Noise};

pub mod common;
use common::{Tuner, cents, span};

/// `docs/AUDIO_ENGINE.md` §1: synthetic steady tones, 30–1400 Hz.
const BUDGET_CENTS: f64 = 0.5;
/// The crossover must leave the budget this many times over.
const MARGIN: f64 = 2.0;
const LOWEST_HZ: f64 = 30.0;
const HIGHEST_HZ: f64 = 1_400.0;
const POINTS_PER_OCTAVE: u32 = 24;
const PHASES: usize = 8;
/// The latency criterion: within ±5 ¢, and staying there.
const SETTLED_CENTS: f64 = 5.0;
/// Whole-period frequencies are swept this far either side, in steps of a tenth of a hertz: the
/// error spikes there are a few tenths of a hertz wide.
const SPIKE_HALF_WIDTH_HZ: f64 = 0.3;

type Generator = fn(&mut [f32], f64, u32, f64, f64);

const SIGNALS: [(&str, Generator); 2] = [("sine", synth::sine), ("sawtooth", synth::sawtooth)];

/// Writes a line of the summary, only under `--nocapture` (see `tests/accuracy.rs`: `println!` is
/// on the crate's real-time deny list, which covers tests too).
fn report(line: fmt::Arguments<'_>) {
    let asked = std::env::args().any(|arg| arg == "--nocapture" || arg == "--show-output")
        || std::env::var_os("RUST_TEST_NOCAPTURE").is_some();
    if asked {
        writeln!(std::io::stdout().lock(), "{line}").unwrap();
    }
}

/// `LOWEST_HZ · 2^(i/24)` up to `HIGHEST_HZ`, which is always included.
fn sweep(points_per_octave: u32) -> Vec<f64> {
    (0..)
        .map(|i| LOWEST_HZ * 2_f64.powf(f64::from(i) / f64::from(points_per_octave)))
        .take_while(|&hz| hz < HIGHEST_HZ)
        .chain(std::iter::once(HIGHEST_HZ))
        .collect()
}

fn phase(index: usize) -> f64 {
    TAU * index_to_f64(index) / index_to_f64(PHASES)
}

fn samples(seconds: f64, rate: u32) -> usize {
    index_from_f64((seconds * f64::from(rate)).ceil())
}

/// The frequencies at which `range`'s window holds a whole number of periods, `k · rate / window`,
/// within `[from, to]`, each with its neighbours to ±`SPIKE_HALF_WIDTH_HZ` in 0.1 Hz steps. Before
/// `T-003b`'s common-overlap step 5 the sawtooth's error spiked there (−0.514 ¢ at 75.02 Hz, 44.1
/// kHz, Mid), between the points of any octave grid.
fn whole_periods(table: &RangeTable, range: Range, from: f64, to: f64) -> Vec<f64> {
    let spec = table.spec(range);
    let spacing = f64::from(spec.analysis_rate) / index_to_f64(spec.window);
    (1..)
        .map(|k| spacing * f64::from(k))
        .take_while(|&hz| hz <= to + SPIKE_HALF_WIDTH_HZ)
        .flat_map(|hz| (-3..=3).map(move |step| hz + 0.1 * f64::from(step)))
        .filter(|&hz| (from..=to).contains(&hz))
        .collect()
}

fn input_hop(table: &RangeTable, range: Range) -> usize {
    let spec = table.spec(range);
    spec.hop
        * match spec.path {
            Path::Decimated => DECIMATION,
            Path::Full => 1,
        }
}

// ── The filters ─────────────────────────────────────────────────────────────────────────────

/// The gain of `filter` at `hz`, measured with a sine of amplitude 1: the RMS of exactly 200 ms
/// of output, after the filter has settled, times √2. 200 ms is a whole number of output samples
/// at both rates (2400 at 12 kHz, 2205 at 11.025 kHz), and every test frequency is a multiple of
/// 5 Hz, so it holds whole periods at the input rate and, aliased or not, at the output's.
fn measured_gain(filter: &mut Decimator, rate: u32, hz: f64) -> f64 {
    filter.reset();
    let factor = filter.factor();
    let settle = filter.taps().len().div_ceil(factor) * factor;
    let outputs = samples(0.2, rate) / factor;
    let mut input = vec![0.0_f32; settle + outputs * factor];
    synth::sine(&mut input, hz, rate, 1.0, 0.3);
    let (mut sum, mut count, mut seen) = (0.0, 0, 0);
    filter.process(&input, |y| {
        seen += 1;
        if seen * factor > settle {
            sum += f64::from(y) * f64::from(y);
            count += 1;
        }
    });
    assert_eq!(count, outputs);
    (2.0 * sum / index_to_f64(count)).sqrt()
}

/// `|H(f)| = |Σ h[k]·e^{−j2πfk/fs}|`, exactly, from the taps.
fn exact_gain(taps: &[f64], rate: u32, hz: f64) -> f64 {
    let omega = TAU * hz / f64::from(rate);
    let (re, im) = taps
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (k, h)| {
            let phase = omega * index_to_f64(k);
            (re + h * phase.cos(), im - h * phase.sin())
        });
    re.hypot(im)
}

fn db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

/// Stopband ≥ 80 dB and passband ripple ≤ 0.1 dB for both filters at both rates: exactly, from
/// the taps on a 1 Hz grid (finer than a tenth of any sidelobe, which is `fs/N` ≥ 185 Hz wide),
/// and as the implementation actually filters, measured with sines every 100 Hz, which must agree
/// with the exact response to within 0.1 % or 2·10⁻⁶ (−114 dB) of full scale, the `f32` output's
/// rounding. (In dB the two drift apart near the response's nulls, where both are tiny.) A
/// decimated sine whose alias lands on DC or Nyquist has a level that depends on its phase, so
/// those frequencies are not measured.
#[test]
fn both_low_passes_meet_their_specification() {
    let filters = SUPPORTED_RATES.into_iter().flat_map(|rate| {
        let table = RangeTable::new(rate).unwrap();
        [
            (
                "decimator",
                rate,
                table.decimator().unwrap(),
                table.decimator_spec(),
            ),
            (
                "band-limit",
                rate,
                table.band_limit().unwrap(),
                table.band_limit_spec(),
            ),
        ]
    });
    std::thread::scope(|scope| {
        for (name, rate, mut filter, spec) in filters {
            scope.spawn(move || {
                let taps = filter.taps().to_vec();
                let nyquist = f64::from(rate) / 2.0;
                let grid = |from: f64, to: f64, step: f64| {
                    let first = (from / step).ceil() * step;
                    (0..)
                        .map(move |i| first + step * f64::from(i))
                        .take_while(move |&hz| hz <= to)
                };
                let ripple = grid(1.0, spec.passband_hz, 1.0)
                    .map(|hz| db(exact_gain(&taps, rate, hz)).abs())
                    .fold(0.0, f64::max);
                let (stopband, worst_hz) = grid(spec.stopband_hz, nyquist, 1.0)
                    .map(|hz| (-db(exact_gain(&taps, rate, hz)), hz))
                    .fold((f64::INFINITY, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                let alias = nyquist / index_to_f64(filter.factor());
                let mut disagreement: f64 = 0.0;
                for hz in grid(100.0, nyquist - 100.0, 100.0) {
                    if filter.factor() > 1 && (hz / alias).fract() == 0.0 {
                        continue;
                    }
                    let exact = exact_gain(&taps, rate, hz);
                    let excess =
                        (measured_gain(&mut filter, rate, hz) - exact).abs() - 1e-3 * exact;
                    disagreement = disagreement.max(excess);
                }
                report(format_args!(
                    "{name} at {rate} Hz: {} taps, group delay {} samples; passband to {:.0} Hz, \
                     ripple {ripple:.5} dB; stopband from {:.0} Hz, at least {stopband:.2} dB \
                     (at {worst_hz:.0} Hz); sines agree to {disagreement:.1e} beyond 0.1 %",
                    taps.len(),
                    filter.group_delay(),
                    spec.passband_hz,
                    spec.stopband_hz,
                ));
                assert!(ripple <= 0.1, "{name} at {rate} Hz: ripple {ripple} dB");
                assert!(
                    stopband >= 80.0,
                    "{name} at {rate} Hz: {stopband} dB at {worst_hz} Hz"
                );
                assert!(
                    disagreement <= 2e-6,
                    "{name} at {rate} Hz: sines off by {disagreement}"
                );
            });
        }
    });
}

// ── The crossover probe ─────────────────────────────────────────────────────────────────────

/// The worst error for a tone through the ×4 path alone, analysed in a window of `window`
/// samples at the decimated rate: over eight phases and two frame alignments, a decimated sample
/// apart.
fn decimated_error(rate: u32, generate: Generator, hz: f64, window: usize) -> f64 {
    let table = RangeTable::new(rate).unwrap();
    let mut decimator = table.decimator().unwrap();
    let mut mpm = Mpm::new(MpmConfig::with_window(window)).unwrap();
    let longest = 2 * decimator.taps().len() + (window + 1) * DECIMATION;
    let mut input = vec![0.0_f32; longest];
    let mut output = vec![0.0_f32; longest / DECIMATION];
    let mut worst: f64 = 0.0;
    for p in 0..PHASES {
        for alignment in 0..2 {
            let length = longest - alignment * DECIMATION;
            decimator.reset();
            generate(&mut input[..length], hz, rate, 0.5, phase(p));
            let mut len = 0;
            decimator.process(&input[..length], |y| {
                output[len] = y;
                len += 1;
            });
            let error = mpm
                .analyze(&output[len - window..len], rate / 4)
                .map_or(f64::INFINITY, |e| cents(e.frequency_hz, hz));
            if error.abs() > worst.abs() {
                worst = error;
            }
        }
    }
    worst
}

/// The probe's frequencies from `from` to `to`: every quarter of a semitone, and every
/// whole-period frequency of the window with its neighbours ([`whole_periods`]), in order.
fn probe_grid(window: usize, analysis_rate: u32, from: f64, to: f64) -> Vec<f64> {
    let spacing = f64::from(analysis_rate) / index_to_f64(window);
    let mut grid: Vec<f64> = (0..)
        .map(|i| from * 2_f64.powf(f64::from(i) / 48.0))
        .take_while(|&hz| hz <= to)
        .chain(
            (1..)
                .map(|k| spacing * f64::from(k))
                .take_while(|&hz| hz <= to + SPIKE_HALF_WIDTH_HZ)
                .flat_map(|hz| (-3..=3).map(move |step| hz + 0.1 * f64::from(step)))
                .filter(|&hz| (from..=to).contains(&hz)),
        )
        .collect();
    grid.sort_by(f64::total_cmp);
    grid
}

/// The highest probe frequency from `from`, up to `to`, below which the decimated path stays
/// within the budget with [`MARGIN`] in the given window, and the worst error up to it; `None` if
/// it misses at `from` already.
fn decimated_limit(
    rate: u32,
    generate: Generator,
    window: usize,
    from: f64,
    to: f64,
) -> Option<(f64, f64)> {
    let mut limit = None;
    for hz in probe_grid(window, rate / 4, from, to) {
        let error = decimated_error(rate, generate, hz, window).abs();
        if error > BUDGET_CENTS / MARGIN {
            break;
        }
        let worst = limit.map_or(0.0, |(_, worst): (f64, f64)| worst);
        limit = Some((hz, worst.max(error)));
    }
    limit
}

/// How far a probe kept the margin, for its report.
struct Limit(Option<(f64, f64)>);

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some((hz, worst)) => write!(f, "to {hz:.1} Hz (worst {worst:.3} ¢)"),
            None => write!(f, "misses at {CROSSOVER_HZ} Hz"),
        }
    }
}

/// The crossover, measured. Through the ×4 path, in the Mid range's window, sine and sawtooth
/// must stay within the budget with a 2× margin from `CROSSOVER_HZ` to the top of what Mid holds,
/// `CROSSOVER_HZ` raised by the hysteresis. The probe goes on to twice the crossover, every quarter
/// semitone and at every whole-period frequency, and records where the margin ends, if it does.
/// (Below the crossover the headline sweep covers Mid.)
///
/// It also records why High analyses at full rate: the error at 160 Hz in the window High would
/// have at the decimated rate, three periods of 160 Hz (225 samples at 12 kHz, 207 at 11.025).
#[test]
fn the_decimated_path_holds_the_budget_past_the_crossover() {
    let top = CROSSOVER_HZ * HYSTERESIS * (1.0 - 1e-9);
    let cases: Vec<_> = SUPPORTED_RATES
        .into_iter()
        .flat_map(|rate| SIGNALS.map(|signal| (rate, signal)))
        .collect();
    std::thread::scope(|scope| {
        for &(rate, (name, generate)) in &cases {
            scope.spawn(move || {
                let table = RangeTable::new(rate).unwrap();
                let mid = table.spec(Range::Mid).window;
                let short = index_from_f64((3.0 * f64::from(rate / 4) / CROSSOVER_HZ).ceil());
                let limit = decimated_limit(rate, generate, mid, CROSSOVER_HZ, 2.0 * CROSSOVER_HZ);
                let at_floor = decimated_error(rate, generate, CROSSOVER_HZ, short);
                report(format_args!(
                    "probe, {name} at {rate} Hz, ×4, within {:.2} ¢: Mid's {mid}-sample window {} \
                     (probed to {:.0} Hz); a {short}-sample window reads {at_floor:+.3} ¢ at \
                     {CROSSOVER_HZ} Hz",
                    BUDGET_CENTS / MARGIN,
                    Limit(limit),
                    2.0 * CROSSOVER_HZ,
                ));
                assert!(
                    limit.is_some_and(|(hz, _)| hz >= top),
                    "{name} at {rate} Hz: the decimated path keeps the margin only {}, short of \
                     the crossover's top, {top:.1} Hz",
                    Limit(limit)
                );
            });
        }
    });
}

// ── The headline gate ───────────────────────────────────────────────────────────────────────

/// One frequency, in one range, at one phase: the worst of the frames after warm-up.
#[derive(Debug, Clone, Copy)]
struct Case {
    hz: f64,
    phase: usize,
    range: Range,
    cents: f64,
    clarity: f32,
    frames: usize,
}

/// Stream a steady tone through the chain, held in `range`, and judge every frame whose input is
/// all past the warm-up: the DC block's −90 dB settling time, so its onset transient is gone. The
/// analysis is told the gate is closed until the first judged frame is near, so that it does not
/// spend a debug build's time on frames nobody judges; the front end still filters every sample.
fn steady_case(rate: u32, generate: Generator, hz: f64, phase_index: usize, range: Range) -> Case {
    const BLOCK: usize = 256;
    let mut tuner = Tuner::new(rate, Selection::Fixed(range));
    let warmup = u64::try_from(DcBlock::new(rate).unwrap().settling_samples(90.0)).unwrap();
    let first = warmup + span(tuner.table(), range);
    let length = usize::try_from(first).unwrap() + 4 * input_hop(tuner.table(), range);
    let mut signal = vec![0.0_f32; length];
    generate(&mut signal, hz, rate, 0.5, phase(phase_index));
    let mut case = Case {
        hz,
        phase: phase_index,
        range,
        cents: 0.0,
        clarity: 1.0,
        frames: 0,
    };
    for block in signal.chunks_mut(BLOCK) {
        let near = tuner.analysis.position() + u64::try_from(2 * BLOCK).unwrap() >= first;
        let open = tuner.front.process(block) && near;
        tuner.analysis.push(block, open, |frame| {
            if frame.end < first {
                return;
            }
            let (error, clarity) = frame.estimate.map_or((f64::INFINITY, 0.0), |e| {
                (cents(e.frequency_hz, hz), e.clarity)
            });
            case.frames += 1;
            assert_eq!(frame.range, range);
            if error.abs() >= case.cents.abs() {
                (case.cents, case.clarity) = (error, clarity);
            }
        });
    }
    case
}

/// Every (frequency, range) the sweep covers: each frequency of the octave grid in every range
/// that can hold it, and each range's whole-period frequencies ([`whole_periods`]) that it holds.
fn sweep_cases(rate: u32) -> Vec<(f64, Range)> {
    let table = RangeTable::new(rate).unwrap();
    let grid = sweep(POINTS_PER_OCTAVE).into_iter().flat_map(|hz| {
        Range::ALL
            .into_iter()
            .filter(move |&range| table.holds(range, hz))
            .map(move |range| (hz, range))
    });
    let spikes = Range::ALL.into_iter().flat_map(|range| {
        whole_periods(&table, range, LOWEST_HZ, HIGHEST_HZ)
            .into_iter()
            .filter(move |&hz| table.holds(range, hz))
            .map(move |hz| (hz, range))
    });
    grid.chain(spikes).collect()
}

/// The sweep for one signal at one rate, a thread per phase.
fn headline(rate: u32, name: &str, generate: Generator) {
    let cases = sweep_cases(rate);
    let results: Vec<Case> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..PHASES)
            .map(|p| {
                let cases = &cases;
                scope.spawn(move || {
                    cases
                        .iter()
                        .map(|&(hz, range)| steady_case(rate, generate, hz, p, range))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect()
    });
    for range in Range::ALL {
        let in_range = results.iter().filter(|c| c.range == range);
        if let Some(worst) = in_range.max_by(|a, b| a.cents.abs().total_cmp(&b.cents.abs())) {
            report(format_args!(
                "{name} at {rate} Hz, {range:?}: worst {:+.4} ¢ at {:.2} Hz (phase {}, clarity \
                 {:.4})",
                worst.cents, worst.hz, worst.phase, worst.clarity
            ));
        }
    }
    let worst = results
        .iter()
        .max_by(|a, b| a.cents.abs().total_cmp(&b.cents.abs()))
        .unwrap();
    assert!(results.iter().all(|c| c.frames >= 4));
    assert_eq!(results.len(), cases.len() * PHASES);
    assert!(
        worst.cents.abs() <= BUDGET_CENTS,
        "{name} at {:.3} Hz, {:?} range, {rate} Hz: {:+.4} cents, clarity {:.4} (phase {}/8); \
         budget ±{BUDGET_CENTS}",
        worst.hz,
        worst.range,
        worst.cents,
        worst.clarity,
        worst.phase,
    );
}

#[test]
fn the_sweep_covers_every_range_that_can_hold_each_frequency() {
    let points = sweep(POINTS_PER_OCTAVE);
    // 24 · log2(1400/30) = 133.04: i = 0..=133, then 1400 Hz.
    assert_eq!(points.len(), 135);
    for rate in SUPPORTED_RATES {
        let cases = sweep_cases(rate);
        let count = |range| cases.iter().filter(|c| c.1 == range).count();
        // The octave grid: Low to 79.46 Hz, Mid 70.79–169.5 Hz, High from 151.0 Hz, 34 + 30 + 79
        // points. Then seven points round each whole-period frequency a range holds, within
        // 30–1400 Hz: Low's 30, 40, …, 70 Hz (rate/window = 10 Hz; at 44.1 kHz 9.995 Hz, so three
        // points round 29.99 Hz are under 30), Mid's 75, 100, 125, 150 Hz, High's 160, 213.3, …,
        // 1386.7 Hz.
        let low = if rate == 48_000 { 4 } else { 3 } + 4 * 7;
        assert_eq!(
            [count(Range::Low), count(Range::Mid), count(Range::High)],
            [34 + low, 30 + 4 * 7, 79 + 24 * 7]
        );
    }
}

/// **The budget, gated** (`docs/AUDIO_ENGINE.md` §1): a sine through the chain at 48 kHz.
#[test]
fn a_sine_through_the_chain_is_within_half_a_cent_at_48_khz() {
    headline(48_000, "sine", synth::sine);
}

/// **The budget, gated**: a sawtooth, every harmonic to Nyquist, through the chain at 48 kHz.
/// Without the band-limit it misses by up to 2.7 ¢ near 1400 Hz (`T-003a`, Deviation 2).
#[test]
fn a_sawtooth_through_the_chain_is_within_half_a_cent_at_48_khz() {
    headline(48_000, "sawtooth", synth::sawtooth);
}

/// **The budget, gated**: a sine at 44.1 kHz.
#[test]
fn a_sine_through_the_chain_is_within_half_a_cent_at_44_1_khz() {
    headline(44_100, "sine", synth::sine);
}

/// **The budget, gated**: a sawtooth at 44.1 kHz.
#[test]
fn a_sawtooth_through_the_chain_is_within_half_a_cent_at_44_1_khz() {
    headline(44_100, "sawtooth", synth::sawtooth);
}

// ── Latency ─────────────────────────────────────────────────────────────────────────────────

/// Input samples from the onset of a tone, after silence, to the end of the first frame from
/// which every estimate is within ±5 ¢. `None` if the last frame is not.
fn onset_latency(
    rate: u32,
    range: Range,
    generate: Generator,
    hz: f64,
    phase_index: usize,
    onset: usize,
) -> Option<u64> {
    let mut chain = Tuner::new(rate, Selection::Fixed(range));
    let ledger = chain.table().ledger(range);
    let length = onset + samples(ledger.chain_seconds() + 0.1, rate);
    let mut signal = vec![0.0_f32; length];
    generate(&mut signal[onset..], hz, rate, 0.5, phase(phase_index));
    let onset = u64::try_from(onset).unwrap();
    let mut settled: Option<u64> = None;
    for block in signal.chunks(192) {
        chain.push(block, |event| {
            let good = event
                .estimate
                .is_some_and(|e| cents(e.frequency_hz, hz).abs() <= SETTLED_CENTS);
            if !good {
                settled = None;
            } else if settled.is_none() && event.end > onset {
                settled = Some(event.end - onset);
            }
        });
    }
    settled
}

/// Each range's onset latency, measured on the audio clock, within its line of the ledger
/// (`docs/AUDIO_ENGINE.md` §1): group delay + window + settling + one hop. The worst case over
/// four phases and four onsets spread across a hop; "stays" means for 100 ms past the ledger's
/// line.
#[test]
fn onset_latency_is_within_the_ledger() {
    // Each range's floor, where the window is shortest in periods, and its slowest notes; the
    // DC block's transient is worst nearest its corner, at low B.
    let notes: [(Range, &[f64]); 3] = [
        (Range::Low, &[30.87, 41.2]),
        (Range::Mid, &[75.0, 82.41, 146.83]),
        (Range::High, &[160.0, 329.63, 1_400.0]),
    ];
    for rate in SUPPORTED_RATES {
        let table = RangeTable::new(rate).unwrap();
        let silence = samples(0.05, rate);
        for (range, frequencies) in notes {
            let ledger = table.ledger(range);
            let limit = ledger.chain_seconds() * f64::from(rate);
            let hop = input_hop(&table, range);
            for &hz in frequencies {
                for (name, generate) in SIGNALS {
                    let worst = std::thread::scope(|scope| {
                        let handles: Vec<_> = (0..PHASES)
                            .step_by(2)
                            .flat_map(|p| (0..4).map(move |o| (p, silence + o * hop / 4)))
                            .map(|(p, onset)| {
                                scope.spawn(move || {
                                    onset_latency(rate, range, generate, hz, p, onset)
                                })
                            })
                            .collect();
                        handles
                            .into_iter()
                            .map(|handle| {
                                handle.join().unwrap().unwrap_or_else(|| {
                                    panic!("{name} {hz} Hz, {range:?}, {rate} Hz: never settles")
                                })
                            })
                            .max()
                            .unwrap()
                    });
                    let worst = index_to_f64(usize::try_from(worst).unwrap());
                    report(format_args!(
                        "latency, {range:?} at {rate} Hz, {name} {hz:7.2} Hz: {:6.2} ms (ledger \
                         {:6.2} ms, total {:6.1} ms)",
                        worst * 1e3 / f64::from(rate),
                        ledger.chain_seconds() * 1e3,
                        ledger.total_seconds() * 1e3,
                    ));
                    assert!(
                        worst <= limit,
                        "{name} {hz} Hz in {range:?} at {rate} Hz: {:.2} ms to settle, ledger \
                         {:.2} ms",
                        worst * 1e3 / f64::from(rate),
                        limit * 1e3 / f64::from(rate)
                    );
                }
            }
        }
    }
}

// ── The selector in the loop ────────────────────────────────────────────────────────────────

/// From the widest range, a steady tone ends in its own range and within the budget there.
#[test]
fn from_home_the_selector_finds_the_range_of_each_note() {
    for rate in SUPPORTED_RATES {
        let table = RangeTable::new(rate).unwrap();
        for hz in [41.2, 82.41, 196.0, 659.26, 1_400.0] {
            for (name, generate) in SIGNALS {
                let mut chain = Tuner::new(rate, Selection::Auto(Range::Low));
                let mut signal = vec![0.0_f32; samples(0.6, rate)];
                generate(&mut signal, hz, rate, 0.5, 0.4);
                let mut last = None;
                let mut arrived: Option<u64> = None;
                for block in signal.chunks(256) {
                    chain.push(block, |event| {
                        if event.range == table.range_for(hz) {
                            arrived.get_or_insert(event.end);
                        } else {
                            arrived = None;
                        }
                        last = Some(*event);
                    });
                }
                let last = last.unwrap();
                let error = last
                    .estimate
                    .map_or(f64::INFINITY, |e| cents(e.frequency_hz, hz));
                let arrived =
                    index_to_f64(usize::try_from(arrived.unwrap_or(u64::MAX)).unwrap_or(0));
                report(format_args!(
                    "selector at {rate} Hz, {name} {hz:7.2} Hz: in {:?} after {:6.1} ms, last \
                     frame {error:+.4} ¢",
                    last.range,
                    arrived * 1e3 / f64::from(rate)
                ));
                assert_eq!(
                    last.range,
                    table.range_for(hz),
                    "{name} {hz} Hz at {rate} Hz"
                );
                assert!(
                    error.abs() <= BUDGET_CENTS,
                    "{name} {hz} Hz at {rate} Hz: {error} ¢"
                );
            }
        }
    }
}

// ── The octave above ────────────────────────────────────────────────────────────────────────

/// A low note with a weak fundamental, as a guitar's low E often is: partials 1, 2 and 3 at
/// relative levels 0.3, 1 and 0.4 (the reviewer's probe), summed to peak `amplitude` at most.
fn weak_fundamental(out: &mut [f32], f0: f64, rate: u32, amplitude: f64, phase: f64) {
    let mut partial = vec![0.0_f32; out.len()];
    out.fill(0.0);
    for (n, level) in [(1.0, 0.3), (2.0, 1.0), (3.0, 0.4)] {
        synth::sine(
            &mut partial,
            n * f0,
            rate,
            amplitude * level / 1.7,
            n * phase,
        );
        for (o, p) in out.iter_mut().zip(&partial) {
            *o += p;
        }
    }
}

/// One note, then `gap` seconds of silence, then another, streamed through the chain under the
/// selector from `home`. Returns, for the second note, the time from its onset to the first frame
/// from which every frame is in `expected` within ±0.5 ¢ of `f0`, and the last frame. `None` if
/// that never happens.
fn after_a_change(
    rate: u32,
    first: (Generator, f64),
    gap: f64,
    second: (Generator, f64),
    expected: Range,
) -> (Option<f64>, Option<diapason_dsp::chain::Frame>) {
    let mut tuner = Tuner::new(rate, Selection::Auto(Range::Low));
    let (lead, silence, length) = (samples(0.5, rate), samples(gap, rate), samples(1.0, rate));
    let mut signal = vec![0.0_f32; lead + silence + length];
    (first.0)(&mut signal[..lead], first.1, rate, 0.5, 0.0);
    (second.0)(&mut signal[lead + silence..], second.1, rate, 0.5, 0.0);
    let onset = u64::try_from(lead + silence).unwrap();
    let (mut settled, mut last) = (None, None);
    tuner.push(&signal, |frame| {
        last = Some(*frame);
        let good = frame.range == expected
            && frame
                .estimate
                .is_some_and(|e| cents(e.frequency_hz, second.1).abs() <= BUDGET_CENTS);
        if !good || frame.end <= onset {
            settled = None;
        } else if settled.is_none() {
            settled = Some(frame.end - onset);
        }
    });
    let ms = settled.map(|n| index_to_f64(usize::try_from(n).unwrap()) * 1e3 / f64::from(rate));
    (ms, last)
}

/// `Mpm` searches lags up to half its window, so High cannot see a low E (82.41 Hz); with a weak
/// fundamental it reads the second partial, 164.8 Hz, which High holds. Before `T-003b`'s Low
/// reference frames the chain stayed there, 1206 ¢ sharp, for as long as the note rang. Now the
/// Low range, analysed alongside, sees 82.41 Hz and moves the chain to Mid. With and without a
/// gap shorter than the gate's hold, so the selector never goes home in between.
#[test]
fn a_low_e_after_a_high_one_is_not_read_an_octave_up() {
    for rate in SUPPORTED_RATES {
        for gap in [0.0, 0.1] {
            let first = (synth::sine as Generator, 329.63);
            let second = (weak_fundamental as Generator, 82.41);
            let (settled, last) = after_a_change(rate, first, gap, second, Range::Mid);
            let last = last.unwrap();
            report(format_args!(
                "octave guard at {rate} Hz, E4 then low E, gap {gap} s: settled in Mid after \
                 {settled:?} ms; last frame {:?} at {:?} Hz",
                last.range,
                last.estimate.map(|e| e.frequency_hz)
            ));
            let settled = settled.unwrap_or_else(|| {
                panic!("{rate} Hz, gap {gap} s: low E never settled in Mid; last frame {last:?}")
            });
            assert!(settled <= 250.0, "{rate} Hz, gap {gap} s: {settled} ms");
        }
    }
}

/// The same from Mid: a bass E (41.2 Hz, weak fundamental) after an A2. Mid cannot see below
/// 50 Hz, reads the bass E's second partial (81–84 Hz at a clarity near 0.61 before `T-003b`'s
/// fix), and its sporadic clear frames kept the selector from scanning.
#[test]
fn a_bass_e_after_an_a2_is_not_read_an_octave_up() {
    for rate in SUPPORTED_RATES {
        let first = (synth::sawtooth as Generator, 110.0);
        let second = (weak_fundamental as Generator, 41.2);
        let (settled, last) = after_a_change(rate, first, 0.0, second, Range::Low);
        report(format_args!(
            "octave guard at {rate} Hz, A2 then bass E: settled in Low after {settled:?} ms"
        ));
        let settled = settled.unwrap_or_else(|| {
            panic!("{rate} Hz: bass E never settled in Low; last frame {last:?}")
        });
        assert!(settled <= 250.0, "{rate} Hz: {settled} ms");
    }
}

// ── The gate in a loud room ─────────────────────────────────────────────────────────────────

/// `samples` of uniform noise whose RMS is `dbfs`.
fn room(dbfs: f64, seconds: f64, rate: u32, seed: u64) -> Vec<f32> {
    let mut out = vec![0.0_f32; samples(seconds, rate)];
    Noise::new(seed).fill(&mut out, 10_f64.powf(dbfs / 20.0) * 3_f64.sqrt());
    out
}

/// `docs/PRODUCT_SPEC.md` §2: ambient noise raises the gate. Room noise at −45 dBFS, above the
/// −50 dBFS minimum, opens the gate from the first block. After `UNPITCHED_SECONDS` without a clear
/// pitch the analysis says so, the floor learns the room, and the gate closes within 2 s more.
/// (Held in one range to keep a debug build quick: the report is the same under the selector.)
#[test]
fn a_loud_room_raises_the_gate_and_closes_it() {
    for rate in SUPPORTED_RATES {
        let mut tuner = Tuner::new(rate, Selection::Fixed(Range::Mid));
        let noise = room(-45.0, UNPITCHED_SECONDS + 2.0, rate, 3);
        let block = samples(0.01, rate);
        let mut closed_at = None;
        for (i, chunk) in noise.chunks(block).enumerate() {
            tuner.push(chunk, |_| {});
            if i == 0 {
                assert!(tuner.gate_is_open(), "{rate} Hz: −45 dBFS opens the gate");
            }
            if !tuner.gate_is_open() && closed_at.is_none() {
                closed_at = Some(index_to_f64(i + 1) * 0.01);
            }
        }
        let threshold = tuner.front.gate().open_threshold_dbfs();
        report(format_args!(
            "loud room at {rate} Hz: closed after {closed_at:?} s, open threshold {threshold:.1} dBFS"
        ));
        assert!(
            !tuner.gate_is_open() && threshold > -45.0,
            "{rate} Hz: {threshold} dBFS"
        );
        assert!(closed_at.is_some_and(|s| s > UNPITCHED_SECONDS));
    }
}

/// And the counterpart that makes learning while open safe: a held note keeps a clear pitch, so
/// the analysis never calls it noise, and the gate stays open throughout. Five seconds is more
/// than twice what a gate that learned the note would last (it closed at 2.26 s, `T-003b`); the
/// gate's own ten-second test is `gate::tests`.
#[test]
fn a_held_note_keeps_the_gate_open_through_the_chain() {
    for rate in SUPPORTED_RATES {
        let mut tuner = Tuner::new(rate, Selection::Fixed(Range::High));
        tuner.push(&room(-65.0, 1.0, rate, 4), |_| {});
        let mut note = vec![0.0_f32; samples(5.0, rate)];
        // −30 dBFS RMS: a sawtooth's RMS is its peak over √3.
        synth::sawtooth(
            &mut note,
            196.0,
            rate,
            10_f64.powf(-30.0 / 20.0) * 3_f64.sqrt(),
            0.0,
        );
        let block = samples(0.01, rate);
        for (i, chunk) in note.chunks(block).enumerate() {
            tuner.push(chunk, |_| {});
            assert!(
                tuner.gate_is_open(),
                "{rate} Hz: closed after {} s",
                index_to_f64(i) * 0.01
            );
        }
    }
}

//! The tuner's analysis chain, end to end, on synthetic fixtures (`docs/AUDIO_ENGINE.md` §1–§2,
//! `docs/adr/0028-windows-hops-from-the-latency-ledger.md`, `docs/TESTING.md` §2).
//!
//! Every signal is *streamed* through the chain of `common::Chain`, block by block, exactly as the
//! engine will feed it: DC block, gate, band-limit or ×4 decimation, frames, `Mpm`. Nothing hands
//! a frame to the detector directly. Each test runs at every rate in `SUPPORTED_RATES`.
//!
//! - The filters meet their specification, measured with sines.
//! - The decimated path's accuracy, probed and recorded: where the crossover can sit.
//! - **The headline gate:** sine and sawtooth within ±0.5 ¢ over 30–1400 Hz, 24 points per
//!   octave, 8 phases, in every range that can hold each frequency.
//! - Onset latency, per range, within its line of the ledger.
//! - From its home range, the selector finds the range of each note.
//!
//! `cargo test -p diapason_dsp --test chain -- --nocapture` prints the measured tables.

use std::f64::consts::TAU;
use std::fmt;
use std::io::Write;

use diapason_dsp::convert::{index_from_f64, index_to_f64};
use diapason_dsp::filter::{DcBlock, Decimator};
use diapason_dsp::pitch::{Mpm, MpmConfig};
use diapason_dsp::range::{
    CROSSOVER_HZ, DECIMATION, HYSTERESIS, Path, Range, RangeTable, SUPPORTED_RATES,
};
use diapason_dsp::synth;

pub mod common;
use common::{Chain, Selection, cents};

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

fn input_hop(table: &RangeTable, range: Range) -> usize {
    let spec = table.spec(range);
    spec.hop
        * match spec.path {
            Path::Decimated => DECIMATION,
            Path::Full => 1,
        }
}

// ── The filters ─────────────────────────────────────────────────────────────────────────────

/// The gain of `filter` at `hz`, from a sine of amplitude 1: the RMS of exactly 50 ms of output,
/// after the filter has settled, times √2. Every test frequency is a multiple of 20 Hz, so 50 ms
/// holds whole periods at the input rate and at the output's, aliased or not, and the RMS is exact.
fn measured_gain(filter: &mut Decimator, rate: u32, hz: f64) -> f64 {
    filter.reset();
    let settle = filter.taps().len();
    let window = samples(0.05, rate);
    let mut input = vec![0.0_f32; settle + window];
    synth::sine(&mut input, hz, rate, 1.0, 0.3);
    let mut sum = 0.0;
    let mut count = 0;
    let mut seen = 0;
    let factor = filter.factor();
    filter.process(&input, |y| {
        seen += 1;
        if seen * factor > settle {
            sum += f64::from(y) * f64::from(y);
            count += 1;
        }
    });
    (2.0 * sum / index_to_f64(count)).sqrt()
}

/// Stopband ≥ 80 dB and passband ripple ≤ 0.1 dB, measured, for both filters at both rates. The
/// stopband is sampled every 20 Hz, about nine points per sidelobe of the longer filter (whose
/// lobes are `fs/N` = 185 Hz wide), so no lobe's peak is missed by more than a fraction of a dB.
#[test]
fn both_low_passes_meet_their_specification_measured_with_sines() {
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
                let twenties = |from: f64, to: f64| {
                    let first = (from / 20.0).ceil() * 20.0;
                    (0..)
                        .map(move |i| first + 20.0 * f64::from(i))
                        .take_while(move |&hz| hz <= to)
                };
                let ripple = twenties(20.0, spec.passband_hz)
                    .map(|hz| (20.0 * measured_gain(&mut filter, rate, hz).log10()).abs())
                    .fold(0.0, f64::max);
                let (stopband, worst_hz) = twenties(spec.stopband_hz, f64::from(rate) / 2.0 - 20.0)
                    .map(|hz| (-20.0 * measured_gain(&mut filter, rate, hz).log10(), hz))
                    .fold((f64::INFINITY, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                report(format_args!(
                    "{name} at {rate} Hz: {} taps, group delay {} samples, passband to {:.0} Hz \
                     ripple {ripple:.5} dB, stopband from {:.0} Hz at least {stopband:.1} dB \
                     (at {worst_hz:.0} Hz)",
                    filter.taps().len(),
                    filter.group_delay(),
                    spec.passband_hz,
                    spec.stopband_hz,
                ));
                assert!(ripple <= 0.1, "{name} at {rate} Hz: ripple {ripple} dB");
                assert!(
                    stopband >= 80.0,
                    "{name} at {rate} Hz: {stopband} dB at {worst_hz} Hz"
                );
            });
        }
    });
}

// ── The crossover probe ─────────────────────────────────────────────────────────────────────

/// The worst error over the probe's phases for a tone through the ×4 path alone, analysed in a
/// window of `window` samples at the decimated rate.
fn decimated_error(rate: u32, generate: Generator, hz: f64, window: usize) -> f64 {
    let table = RangeTable::new(rate).unwrap();
    let mut decimator = table.decimator().unwrap();
    let mut mpm = Mpm::new(MpmConfig::with_window(window)).unwrap();
    let mut input = vec![0.0_f32; 2 * decimator.taps().len() + window * DECIMATION];
    let mut output = vec![0.0_f32; input.len() / DECIMATION];
    (0..PHASES)
        .step_by(2)
        .map(|p| {
            decimator.reset();
            generate(&mut input, hz, rate, 0.5, phase(p));
            let mut len = 0;
            decimator.process(&input, |y| {
                output[len] = y;
                len += 1;
            });
            mpm.analyze(&output[len - window..len], rate / 4)
                .map_or(f64::INFINITY, |e| cents(e.frequency_hz, hz))
        })
        .fold(
            0.0,
            |worst: f64, c| if c.abs() > worst.abs() { c } else { worst },
        )
}

/// The highest frequency from `from` up to which the decimated path stays within the budget
/// with [`MARGIN`], in the given window, and the worst error up to it.
fn decimated_limit(rate: u32, generate: Generator, window: usize, from: f64) -> (f64, f64) {
    let mut limit: (f64, f64) = (from, 0.0);
    for hz in sweep(12).into_iter().filter(|&hz| hz >= from) {
        let error = decimated_error(rate, generate, hz, window);
        if error.abs() > BUDGET_CENTS / MARGIN {
            break;
        }
        limit = (hz, limit.1.max(error.abs()));
    }
    limit
}

/// The crossover, measured. Through the ×4 path, in the Mid range's window, sine and sawtooth must
/// stay within the budget with a 2× margin up to the top of what Mid holds, `CROSSOVER_HZ`
/// raised by the hysteresis. The probe goes on to where that stops, and records it.
///
/// It also records the window the High range would have at the decimated rate (three periods
/// of 160 Hz at 12 kHz, 225 samples), to show why High analyses at full rate: there the margin
/// is gone just above 160 Hz (`T-003b`).
#[test]
fn the_decimated_path_holds_the_budget_past_the_crossover() {
    let top = CROSSOVER_HZ * HYSTERESIS;
    for rate in SUPPORTED_RATES {
        let table = RangeTable::new(rate).unwrap();
        let mid = table.spec(Range::Mid).window;
        let high_if_decimated = index_from_f64((3.0 * f64::from(rate / 4) / CROSSOVER_HZ).ceil());
        for (name, generate) in SIGNALS {
            let (limit, worst) = decimated_limit(rate, generate, mid, 60.0);
            let (short, _) = decimated_limit(rate, generate, high_if_decimated, CROSSOVER_HZ);
            report(format_args!(
                "probe, {name} at {rate} Hz, ×4: within {:.2} ¢ up to {limit:.0} Hz in Mid's \
                 {mid}-sample window (worst {worst:.3} ¢), up to {short:.0} Hz in a \
                 {high_if_decimated}-sample one",
                BUDGET_CENTS / MARGIN
            ));
            assert!(
                limit >= top,
                "{name} at {rate} Hz: the decimated path keeps the margin only up to {limit:.1} Hz, \
                 below the crossover's top, {top:.1} Hz"
            );
        }
    }
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
/// all past the warm-up: the DC block's −90 dB settling time, so its onset transient is gone.
fn steady_case(rate: u32, generate: Generator, hz: f64, phase_index: usize, range: Range) -> Case {
    let mut chain = Chain::new(rate, Selection::Fixed(range));
    let warmup = u64::try_from(DcBlock::new(rate).unwrap().settling_samples(90.0)).unwrap();
    let first = warmup + chain.span(range);
    chain.analyse_from(first);
    let length = usize::try_from(first).unwrap() + 4 * input_hop(chain.table(), range);
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
    for block in signal.chunks(256) {
        chain.push(block, |event| {
            let (error, clarity) = event.estimate.map_or((f64::INFINITY, 0.0), |e| {
                (cents(e.frequency_hz, hz), e.clarity)
            });
            case.frames += 1;
            assert_eq!(event.range, range);
            if error.abs() >= case.cents.abs() {
                (case.cents, case.clarity) = (error, clarity);
            }
        });
    }
    case
}

/// Every (frequency, range) the sweep covers: each frequency in every range that can hold it.
fn sweep_cases(rate: u32) -> Vec<(f64, Range)> {
    let table = RangeTable::new(rate).unwrap();
    sweep(POINTS_PER_OCTAVE)
        .into_iter()
        .flat_map(|hz| {
            Range::ALL
                .into_iter()
                .filter(move |&range| table.holds(range, hz))
                .map(move |range| (hz, range))
        })
        .collect()
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
        // Low to 79.46 Hz, Mid 70.79–169.5 Hz, High from 151.0 Hz: 34 + 30 + 79 points.
        assert_eq!(
            [count(Range::Low), count(Range::Mid), count(Range::High)],
            [34, 30, 79]
        );
    }
}

/// **The budget, gated** (`docs/AUDIO_ENGINE.md` §1): a sine through the chain at 48 kHz.
#[test]
fn a_sine_through_the_chain_is_within_half_a_cent_at_48_khz() {
    headline(48_000, "sine", synth::sine);
}

/// **The budget, gated**: a sawtooth, every harmonic to Nyquist, through the chain at 48 kHz.
/// Without the band-limit it misses by up to 2.86 ¢ at 1384 Hz (`T-003a`, Deviation 2).
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
    let mut chain = Chain::new(rate, Selection::Fixed(range));
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
    let notes: [(Range, &[f64]); 3] = [
        (Range::Low, &[30.87, 41.2, 55.0]),
        (Range::Mid, &[75.0, 80.0, 82.41, 110.0, 146.83]),
        (Range::High, &[160.0, 196.0, 329.63, 659.26, 1_400.0]),
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
        for hz in [
            30.87, 41.2, 82.41, 110.0, 196.0, 329.63, 659.26, 1_000.0, 1_400.0,
        ] {
            for (name, generate) in SIGNALS {
                let mut chain = Chain::new(rate, Selection::Auto(Range::Low));
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
                        last = Some(event);
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

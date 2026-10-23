//! Per-call cost of the pre-processing chain (`docs/AUDIO_ENGINE.md` §1–§2): what the CPU budget
//! (≤ 8 % of one core while tuning) pays for besides the NSDF.
//!
//! - `chain/decimate` and `chain/band_limit`: one 10 ms block (480 samples at 48 kHz) through
//!   the ×4 decimator and through the full-rate band-limit. Both run on every block.
//! - `chain/frame/low`, `chain/frame/mid`, `chain/frame/high`: one hop of the range at 48 kHz,
//!   pushed into its `FrameAssembler`, then its window analysed by `Mpm`. The selected range pays
//!   this once per hop, so its cost per second is this times `1 / hop`. Since `T-003c` the
//!   ranges below the selected one are analysed too, as verifiers: in High, a frame of each.
//! - `chain/octave`: the octave guard on a frame just analysed, `Mpm::peak_near` at half and
//!   twice the period and `OctaveGuard::judge`: what each selected frame pays on top of
//!   `chain/frame` (Mid's shape, a weak low E, the case whose octave Mid cannot test).
//!
//! `just bench` compares the medians with `benches/baseline.json` (`tools/bench-compare.sh`): the
//! ids are the baseline's keys, so renaming one orphans its baseline.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use diapason_dsp::frame::FrameAssembler;
use diapason_dsp::octave::OctaveGuard;
use diapason_dsp::pitch::{Mpm, MpmConfig};
use diapason_dsp::range::{Path, Range, RangeTable};
use diapason_dsp::synth::{Pluck, WEAK_FUNDAMENTAL, partials, pluck};

const RATE: u32 = 48_000;
const BLOCK: usize = 480;
/// Hops of pluck each frame bench cycles through.
const HOPS: usize = 64;

fn chain(c: &mut Criterion) {
    let table = RangeTable::new(RATE).expect("a supported rate");
    let mut group = c.benchmark_group("chain");

    let mut input = vec![0.0_f32; BLOCK];
    pluck(&mut input, 110.0, RATE, 0.8, &Pluck::default());
    let mut decimator = table.decimator().expect("designed");
    group.bench_function("decimate", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            decimator.process(std::hint::black_box(&input), |y| sum += y);
            sum
        });
    });
    let mut band_limit = table.band_limit().expect("designed");
    group.bench_function("band_limit", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            band_limit.process(std::hint::black_box(&input), |y| sum += y);
            sum
        });
    });

    for (name, range, f0) in [
        ("low", Range::Low, 41.2),
        ("mid", Range::Mid, 110.0),
        ("high", Range::High, 329.63),
    ] {
        let spec = table.spec(range);
        // The analysed stream itself, rendered at the analysis rate (the filters are benched
        // above): a pluck long enough for `HOPS` hops past the first window. Each iteration takes
        // the next hop, wrapping round, so every frame is a real window of the pluck rather than
        // the same samples pushed again.
        let mut stream = vec![0.0_f32; spec.window + HOPS * spec.hop];
        pluck(&mut stream, f0, spec.analysis_rate, 0.8, &Pluck::default());
        let mut frames = FrameAssembler::new(spec.window, spec.hop).expect("valid shape");
        let mut mpm = Mpm::new(MpmConfig::with_window(spec.window)).expect("valid window");
        let (first, hops) = stream.split_at(spec.window);
        frames.push(first, |_, _| {});
        assert!(
            mpm.analyze(frames.window(), spec.analysis_rate).is_some(),
            "the bench frame must exercise the full path, peak picking included"
        );
        assert_eq!(spec.path == Path::Full, range == Range::High);
        let mut next = hops.chunks_exact(spec.hop).cycle();
        group.bench_function(BenchmarkId::new("frame", name), |b| {
            b.iter(|| {
                let hop = next.next().unwrap_or_default();
                let mut estimate = None;
                frames.push(std::hint::black_box(hop), |window, _| {
                    estimate = mpm.analyze(window, spec.analysis_rate);
                });
                estimate
            });
        });
    }

    let mid = table.spec(Range::Mid);
    let mut window = vec![0.0_f32; mid.window];
    partials(
        &mut window,
        82.41,
        mid.analysis_rate,
        0.5,
        0.0,
        &WEAK_FUNDAMENTAL,
    );
    let mut mpm = Mpm::new(MpmConfig::with_window(mid.window)).expect("valid window");
    let pick = mpm
        .analyze(&window, mid.analysis_rate)
        .expect("a weak low E is clear in Mid");
    let lock = mid.window.div_ceil(mid.hop);
    let mut guard = OctaveGuard::new();
    group.bench_function("octave", |b| {
        b.iter(|| {
            let frame = std::hint::black_box(&window);
            let up = mpm.peak_near(frame, mid.analysis_rate, pick.period_samples / 2.0);
            let down = mpm.peak_near(frame, mid.analysis_rate, pick.period_samples * 2.0);
            guard.judge(pick, up, down, lock)
        });
    });
    group.finish();
}

criterion_group!(benches, chain);
criterion_main!(benches);

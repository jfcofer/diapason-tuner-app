//! Per-frame cost of `Mpm::analyze` (`docs/AUDIO_ENGINE.md` §1: a 4096-point NSDF must cost at most
//! 1.5 ms on a 2021 mid-range phone).
//!
//! `just bench` runs this, then `tools/bench-compare.sh` fails on a median more than 10 % above the
//! baseline in `benches/baseline.json`. The group and ids (`nsdf/2048`, `nsdf/4096`, `nsdf/8192`)
//! are the baseline's keys: renaming one orphans its baseline.
//!
//! The frame is a plucked low A string (110 Hz, `B = 10⁻⁴`) 100 ms after the pluck, so the work
//! includes a real key-maximum search, not just the FFTs.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use diapason_dsp::pitch::{Mpm, MpmConfig};
use diapason_dsp::synth::{Pluck, pluck};

const RATE: u32 = 48_000;
const OFFSET: usize = 4_800;

fn nsdf(c: &mut Criterion) {
    let mut group = c.benchmark_group("nsdf");
    for window in [2_048_usize, 4_096, 8_192] {
        let mut signal = vec![0.0_f32; OFFSET + window];
        pluck(&mut signal, 110.0, RATE, 0.8, &Pluck::default());
        let frame = &signal[OFFSET..];
        let mut mpm = Mpm::new(MpmConfig::with_window(window)).expect("valid config");
        assert!(
            mpm.analyze(frame, RATE).is_some(),
            "the bench frame must exercise the full path, peak picking included"
        );
        group.bench_with_input(BenchmarkId::from_parameter(window), frame, |b, frame| {
            b.iter(|| mpm.analyze(std::hint::black_box(frame), RATE));
        });
    }
    group.finish();
}

criterion_group!(benches, nsdf);
criterion_main!(benches);

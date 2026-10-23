//! Analysis ranges: which rate, window and hop the detector uses for a note, how the chain moves
//! between them, and the latency each one costs (`docs/AUDIO_ENGINE.md` §1–§2,
//! `docs/adr/0028-windows-hops-from-the-latency-ledger.md`).
//!
//! Every number here is either derived from the sample rate or measured, and the tests that
//! measure it are named beside it. In short (both supported rates, `T-003b`):
//!
//! - A window holds three periods of its range's floor (`docs/AUDIO_ENGINE.md` §2).
//! - **The crossover is a range boundary, chosen for latency.** Through the ×4 path, in Mid's
//!   480/441-sample window, sine and sawtooth keep a 2× margin (≤ 0.25 ¢) from the crossover to
//!   at least twice it (worst 0.10 ¢, probed to 320 Hz at every quarter semitone and whole-period
//!   frequency: `tests/chain.rs`, `the_decimated_path_holds_the_budget_past_the_crossover`). So
//!   the decimated path could serve higher notes; stretching Mid to them would give them a 40 ms
//!   window instead of High's 18.75 ms. High runs at full rate because the decimator's content
//!   stops at 1.5 kHz and High goes to 1400 Hz. (Before `T-003b`'s common-overlap step 5, a
//!   225-sample decimated window read −0.82 ¢ at 160 Hz; it now reads +0.038 ¢.)
//! - Both paths keep their content below an eighth of their analysis rate, where `T-003a`
//!   measured the parabolic-interpolation bias at 0.0035 lag.
//! - Hops are as short as the CPU budget allows, because each one is paid three times in
//!   latency: one hop to wait for a frame, two more for `T-003c`'s median of five.

use crate::convert::{index_from_f64, index_to_f64};
use crate::filter::{Decimator, FilterError, LowPassSpec};

/// The input rates the chain is designed for. Coefficients, windows and hops are derived from
/// the rate, and the accuracy and latency tests run at each of these.
pub const SUPPORTED_RATES: [u32; 2] = [44_100, 48_000];

/// The decimated path's factor: 48 kHz analysed at 12 kHz, 44.1 kHz at 11.025 kHz.
pub const DECIMATION: usize = 4;

/// Where analysis moves from the decimated path to the full-rate one: the floor of
/// [`Range::High`], in hertz. See the module notes for why it is a range boundary.
pub const CROSSOVER_HZ: f64 = 160.0;

/// Range boundaries are crossed only this far past them: a semitone, `2^(1/12)`, so a note
/// tuned across a boundary does not flip the range back and forth.
pub const HYSTERESIS: f64 = 1.059_463_094_359_295_3;

/// Periods of the range floor a window holds (`docs/AUDIO_ENGINE.md` §2).
pub const PERIODS_PER_WINDOW: f64 = 3.0;

/// Stopband attenuation, and passband ripple `±10^{−A/20}`, both low-pass filters are designed
/// for, in dB. The tests measure at least 80 dB and at most 0.1 dB of ripple.
pub const FILTER_ATTENUATION_DB: f64 = 85.0;

/// The full-rate band-limit's passband edge, in hertz: above the top of the tuner's range
/// (`docs/AUDIO_ENGINE.md` §1, 1400 Hz).
pub const BAND_LIMIT_PASSBAND_HZ: f64 = 1_500.0;

/// The provisional input latency, in seconds, until `T-002f` measures it: one 960-frame burst
/// at 48 kHz, the largest input gap measured on the Redmi (journal `2026-10-21`).
pub const IO_SECONDS: f64 = 0.020;

/// The tuner's snapshot period, in seconds: published at the display rate, 60 Hz, so a new estimate
/// waits up to this long. The owner's decision of 2026-10-23 (`docs/adr/0028`): M3 must publish
/// tuner snapshots at 60 Hz, and this line of the ledger binds it.
pub const SNAPSHOT_SECONDS: f64 = 1.0 / 60.0;

/// One display frame at 60 Hz, in seconds: the snapshot reaches the needle on the next vsync.
pub const DISPLAY_SECONDS: f64 = 1.0 / 60.0;

/// Hops reserved for `T-003c`'s smoothing. A median of five follows a step once three of its
/// five inputs are new: the first new estimate, then two more hops. The one-euro filter adds
/// nothing to the *first* movement, because its output moves on its first new input.
pub const SMOOTHING_HOPS: usize = 2;

/// An analysis range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Range {
    /// From 30 Hz: five- and six-string bass from low B (30.87 Hz) to the D string (73.42 Hz).
    /// Decimated.
    Low,
    /// From 75 Hz: guitar low E (82.41 Hz), A and D (146.83 Hz); the bass's G (98 Hz). Decimated.
    Mid,
    /// From [`CROSSOVER_HZ`]: guitar G3 (196 Hz) and up, violin, ukulele, to 1400 Hz. Full rate.
    High,
}

impl Range {
    /// Every range, lowest first.
    pub const ALL: [Self; 3] = [Self::Low, Self::Mid, Self::High];

    /// The lowest note the range is for, in hertz: its window holds three of its periods.
    #[must_use]
    pub fn floor_hz(self) -> f64 {
        match self {
            Self::Low => 30.0,
            Self::Mid => 75.0,
            Self::High => CROSSOVER_HZ,
        }
    }

    /// The analysis path.
    #[must_use]
    pub fn path(self) -> Path {
        match self {
            Self::Low | Self::Mid => Path::Decimated,
            Self::High => Path::Full,
        }
    }

    /// The hop, in seconds, before rounding to the analysis rate. The ledger pays it three
    /// times; the CPU budget pays its inverse (`T-003b` records frame cost × update rate).
    #[must_use]
    pub fn hop_seconds(self) -> f64 {
        match self {
            Self::Low => 0.005,
            Self::Mid | Self::High => 0.0025,
        }
    }

    /// Allowance for the first estimate to settle once the window holds only the new note, in
    /// seconds, from `tests/chain.rs`, `onset_latency_is_within_the_ledger`. The DC block's
    /// transient (time constant `1/(2π·20 Hz)` = 8 ms) still sits at the start of the window when
    /// it first fills. Measured past group delay, window and hop, worst of both rates: Low 11.9 ms
    /// (low B, a sine, nearest the corner), Mid 3.6 ms (75 Hz). High needs none: its worst,
    /// 21.72 ms at 44.1 kHz, is under its 21.93 ms of group delay, window and hop.
    #[must_use]
    pub fn settling_seconds(self) -> f64 {
        match self {
            Self::Low => 0.015,
            Self::Mid => 0.005,
            Self::High => 0.0,
        }
    }

    /// The next range in a scan, wrapping from the highest to the lowest.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Low => Self::Mid,
            Self::Mid => Self::High,
            Self::High => Self::Low,
        }
    }

    /// The floor of the range above, if there is one.
    fn ceiling_hz(self) -> Option<f64> {
        match self {
            Self::Low => Some(Self::Mid.floor_hz()),
            Self::Mid => Some(Self::High.floor_hz()),
            Self::High => None,
        }
    }
}

/// Which stream a range analyses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Path {
    /// Decimated by [`DECIMATION`] through the anti-alias low-pass.
    Decimated,
    /// At the input rate, through the band-limit low-pass.
    Full,
}

/// One range's analysis shape at one input rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeSpec {
    /// The range.
    pub range: Range,
    /// Which stream it analyses.
    pub path: Path,
    /// That stream's rate, in hertz.
    pub analysis_rate: u32,
    /// Samples per frame at the analysis rate: `⌈3 · rate / floor⌉`.
    pub window: usize,
    /// Samples between frames at the analysis rate: the range's hop, rounded.
    pub hop: usize,
}

impl RangeSpec {
    /// The window's duration, in seconds.
    #[must_use]
    pub fn window_seconds(&self) -> f64 {
        index_to_f64(self.window) / f64::from(self.analysis_rate)
    }

    /// The hop's duration, in seconds.
    #[must_use]
    pub fn hop_seconds(&self) -> f64 {
        index_to_f64(self.hop) / f64::from(self.analysis_rate)
    }
}

/// The latency of one range, stage by stage, in seconds (`docs/AUDIO_ENGINE.md` §1). Worst case:
/// a note can start just after a frame, a snapshot and a vsync.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ledger {
    /// Input I/O, provisional ([`IO_SECONDS`]).
    pub io: f64,
    /// The low-pass filter's group delay on the range's path.
    pub group_delay: f64,
    /// The window filling with the new note.
    pub window: f64,
    /// The estimate settling once it has ([`Range::settling_seconds`]).
    pub settling: f64,
    /// Waiting for the next frame: one hop.
    pub hop: f64,
    /// `T-003c`'s smoothing ([`SMOOTHING_HOPS`]).
    pub smoothing: f64,
    /// Waiting for the next snapshot ([`SNAPSHOT_SECONDS`]).
    pub snapshot: f64,
    /// Waiting for the next display frame ([`DISPLAY_SECONDS`]).
    pub display: f64,
}

impl Ledger {
    /// The part `tests/chain.rs` measures, from the note reaching the chain to the first estimate
    /// that is within ±5 ¢ and stays: group delay, window, settling and one hop.
    #[must_use]
    pub fn chain_seconds(&self) -> f64 {
        self.group_delay + self.window + self.settling + self.hop
    }

    /// Plucked string to moving needle.
    #[must_use]
    pub fn total_seconds(&self) -> f64 {
        self.io + self.chain_seconds() + self.smoothing + self.snapshot + self.display
    }
}

/// The ranges, filters and ledger for one input rate.
///
/// ```
/// use diapason_dsp::range::{Path, Range, RangeTable};
///
/// let table = RangeTable::new(48_000)?;
/// let mid = table.spec(Range::Mid);
/// assert_eq!((mid.path, mid.analysis_rate, mid.window, mid.hop), (Path::Decimated, 12_000, 480, 30));
/// assert_eq!(table.range_for(82.41), Range::Mid);
/// assert!(RangeTable::new(96_000).is_err());
/// # Ok::<(), diapason_dsp::filter::FilterError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeTable {
    sample_rate: u32,
    specs: [RangeSpec; 3],
    decimator_delay: usize,
    band_limit_delay: usize,
}

impl RangeTable {
    /// The table for `sample_rate`, one of [`SUPPORTED_RATES`].
    ///
    /// # Errors
    ///
    /// [`FilterError::SampleRate`] for any other rate.
    pub fn new(sample_rate: u32) -> Result<Self, FilterError> {
        if !SUPPORTED_RATES.contains(&sample_rate) {
            return Err(FilterError::SampleRate(sample_rate));
        }
        let decimated = sample_rate / 4;
        let spec = |range: Range| {
            let analysis_rate = match range.path() {
                Path::Decimated => decimated,
                Path::Full => sample_rate,
            };
            let rate = f64::from(analysis_rate);
            RangeSpec {
                range,
                path: range.path(),
                analysis_rate,
                window: index_from_f64((PERIODS_PER_WINDOW * rate / range.floor_hz()).ceil()),
                hop: index_from_f64((range.hop_seconds() * rate).round()).max(1),
            }
        };
        let mut table = Self {
            sample_rate,
            specs: Range::ALL.map(spec),
            decimator_delay: 0,
            band_limit_delay: 0,
        };
        table.decimator_delay = table.decimator()?.group_delay();
        table.band_limit_delay = table.band_limit()?.group_delay();
        Ok(table)
    }

    /// The input rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The shape of `range`.
    #[must_use]
    pub fn spec(&self, range: Range) -> RangeSpec {
        let [low, mid, high] = self.specs;
        match range {
            Range::Low => low,
            Range::Mid => mid,
            Range::High => high,
        }
    }

    /// The decimated path's anti-alias low-pass: content kept below an eighth of the decimated
    /// rate (1.5 kHz at 12 kHz), passband to a twenty-fourth (500 Hz), which holds the Mid
    /// range's top, `CROSSOVER_HZ · HYSTERESIS`, with room to spare.
    #[must_use]
    pub fn decimator_spec(&self) -> LowPassSpec {
        let decimated = f64::from(self.sample_rate) / index_to_f64(DECIMATION);
        LowPassSpec {
            sample_rate: self.sample_rate,
            passband_hz: decimated / 24.0,
            stopband_hz: decimated / 8.0,
            attenuation_db: FILTER_ATTENUATION_DB,
        }
    }

    /// The full-rate path's band-limit: content kept below an eighth of the input rate (6 kHz
    /// at 48 kHz), passband to [`BAND_LIMIT_PASSBAND_HZ`].
    #[must_use]
    pub fn band_limit_spec(&self) -> LowPassSpec {
        LowPassSpec {
            sample_rate: self.sample_rate,
            passband_hz: BAND_LIMIT_PASSBAND_HZ,
            stopband_hz: f64::from(self.sample_rate) / 8.0,
            attenuation_db: FILTER_ATTENUATION_DB,
        }
    }

    /// A ×[`DECIMATION`] decimator built from [`Self::decimator_spec`].
    ///
    /// # Errors
    ///
    /// Whatever the design rejects; nothing, for a table that was built.
    pub fn decimator(&self) -> Result<Decimator, FilterError> {
        Decimator::new(&self.decimator_spec(), DECIMATION)
    }

    /// The full-rate band-limit, built from [`Self::band_limit_spec`].
    ///
    /// # Errors
    ///
    /// Whatever the design rejects; nothing, for a table that was built.
    pub fn band_limit(&self) -> Result<Decimator, FilterError> {
        Decimator::new(&self.band_limit_spec(), 1)
    }

    /// The group delay of `path`'s filter, in input samples.
    #[must_use]
    pub fn group_delay(&self, path: Path) -> usize {
        match path {
            Path::Decimated => self.decimator_delay,
            Path::Full => self.band_limit_delay,
        }
    }

    /// The range a note at `hz` belongs to, with no history: the highest range whose floor it
    /// reaches, or [`Range::Low`].
    #[must_use]
    pub fn range_for(&self, hz: f64) -> Range {
        Range::ALL
            .into_iter()
            .rev()
            .find(|range| hz >= range.floor_hz())
            .unwrap_or(Range::Low)
    }

    /// Whether `range` keeps a note at `hz` once it is selected: its band widened by
    /// [`HYSTERESIS`] on each side that borders another range.
    #[must_use]
    pub fn holds(&self, range: Range, hz: f64) -> bool {
        let above_floor = range == Range::Low || hz >= range.floor_hz() / HYSTERESIS;
        let below_ceiling = range
            .ceiling_hz()
            .is_none_or(|ceiling| hz < ceiling * HYSTERESIS);
        hz.is_finite() && hz > 0.0 && above_floor && below_ceiling
    }

    /// The latency ledger of `range` at this rate.
    #[must_use]
    pub fn ledger(&self, range: Range) -> Ledger {
        let spec = self.spec(range);
        let hop = spec.hop_seconds();
        Ledger {
            io: IO_SECONDS,
            group_delay: index_to_f64(self.group_delay(spec.path)) / f64::from(self.sample_rate),
            window: spec.window_seconds(),
            settling: range.settling_seconds(),
            hop,
            smoothing: index_to_f64(SMOOTHING_HOPS) * hop,
            snapshot: SNAPSHOT_SECONDS,
            display: DISPLAY_SECONDS,
        }
    }
}

/// What one frame told the [`RangeSelector`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Observation {
    /// A clear estimate, in hertz.
    Pitch(f64),
    /// The gate is open but the frame had no clear pitch.
    Unclear,
    /// The gate is closed.
    Silent,
}

/// Seconds without a clear estimate, past one window, before the selector tries the next range.
pub const MISS_GRACE_SECONDS: f64 = 0.050;

/// How close, in cents, the current range's pitch must be to a whole multiple of the Low range's
/// for [`RangeSelector::observe_reference`] to take the current one for a harmonic: half a
/// semitone, wide enough for a stiff string's stretched partials.
pub const HARMONIC_TOLERANCE_CENTS: f64 = 50.0;

/// Chooses the range for the next frame from what the last one found. A pure state machine:
/// no clock but the frames it is shown, no I/O, no allocation.
///
/// - A clear pitch the current range [`holds`](RangeTable::holds) keeps it. One outside moves to
///   [`RangeTable::range_for`] the pitch. The bands overlap by [`HYSTERESIS`], so a note tuned
///   across a boundary stays where it is.
/// - Silence (a closed gate) returns to the home range, the one a preset picks (M3).
/// - Each frame without a clear pitch, gate open, adds a miss; each clear one takes a miss away
///   (a decay, not a reset, so sporadic clear frames cannot hold a range that mostly sees
///   nothing). After a window and [`MISS_GRACE_SECONDS`] of misses it tries the next range
///   ([`Range::next`]). That is how a note no range can see from home (1.4 kHz is in the
///   decimated path's stopband) is found: each range's history is kept full, so a switch costs
///   one hop, not a window.
/// - **The octave guard of range selection.** `Mpm` searches lags up to half its window, so a
///   range cannot see below about 1.5 periods of its floor: High is blind below 107 Hz, Mid below
///   50 Hz. A low E (82.41 Hz) heard by High reads as its second harmonic, 164.8 Hz, which High
///   holds. So the Low range, which sees down to 20 Hz, is analysed as a reference while another
///   range is selected ([`Self::observe_reference`]): a clear Low pitch below the current band,
///   with the current pitch absent or a whole multiple of it, moves to the Low pitch's range.
///
/// ```
/// use diapason_dsp::range::{Observation, Range, RangeSelector, RangeTable};
///
/// let table = RangeTable::new(48_000)?;
/// let mut selector = RangeSelector::new(&table, Range::Low);
/// assert_eq!(selector.observe(Observation::Pitch(82.41)), Range::Mid);
/// assert_eq!(selector.observe(Observation::Pitch(72.0)), Range::Mid); // within the hysteresis
/// assert_eq!(selector.observe(Observation::Pitch(164.82)), Range::Mid);
/// assert_eq!(selector.observe(Observation::Pitch(330.0)), Range::High);
/// assert_eq!(selector.observe(Observation::Pitch(164.82)), Range::High); // the octave above...
/// assert_eq!(selector.observe_reference(Some(82.41)), Range::Mid); // ...caught by Low
/// assert_eq!(selector.observe(Observation::Silent), Range::Low);
/// # Ok::<(), diapason_dsp::filter::FilterError>(())
/// ```
#[derive(Debug, Clone)]
pub struct RangeSelector {
    table: RangeTable,
    home: Range,
    current: Range,
    misses: usize,
    /// The current range's latest clear pitch, if its latest frame had one.
    pitch: Option<f64>,
}

impl RangeSelector {
    /// A selector over `table`, starting at, and returning to, `home`.
    #[must_use]
    pub fn new(table: &RangeTable, home: Range) -> Self {
        Self {
            table: *table,
            home,
            current: home,
            misses: 0,
            pitch: None,
        }
    }

    /// The range for the next frame.
    #[must_use]
    pub fn current(&self) -> Range {
        self.current
    }

    /// Frames without a clear pitch, gate open, before `range` gives way to the next.
    #[must_use]
    pub fn miss_limit(&self, range: Range) -> usize {
        let spec = self.table.spec(range);
        let grace = index_from_f64((MISS_GRACE_SECONDS * f64::from(spec.analysis_rate)).round());
        (spec.window + grace).div_ceil(spec.hop.max(1))
    }

    fn switch(&mut self, range: Range) {
        if range != self.current {
            self.current = range;
            self.misses = 0;
            self.pitch = None;
        }
    }

    /// Take one frame's outcome; return the range for the next frame.
    pub fn observe(&mut self, observation: Observation) -> Range {
        match observation {
            Observation::Pitch(hz) if hz.is_finite() && hz > 0.0 => {
                self.misses = self.misses.saturating_sub(1);
                self.pitch = Some(hz);
                if !self.table.holds(self.current, hz) {
                    self.switch(self.table.range_for(hz));
                }
            }
            Observation::Silent => {
                self.misses = 0;
                self.pitch = None;
                self.current = self.home;
            }
            Observation::Pitch(_) | Observation::Unclear => {
                self.pitch = None;
                self.misses += 1;
                if self.misses >= self.miss_limit(self.current) {
                    self.switch(self.current.next());
                    self.misses = 0;
                }
            }
        }
        self.current
    }

    /// Take a frame of the Low range, analysed as a reference while another range is selected;
    /// return the range for the next frame. A clear Low pitch `p` below the current range's band
    /// moves to `p`'s range when the current range has no clear pitch, or one within
    /// [`HARMONIC_TOLERANCE_CENTS`] of `k·p` for a whole `k ≥ 2`. Anything else changes nothing:
    /// a melody over a bass note keeps its range.
    pub fn observe_reference(&mut self, low: Option<f64>) -> Range {
        let Some(low) = low.filter(|hz| hz.is_finite() && *hz > 0.0) else {
            return self.current;
        };
        let below = low < self.current.floor_hz() && !self.table.holds(self.current, low);
        if self.current != Range::Low && below && self.pitch.is_none_or(|hz| harmonic(hz, low)) {
            self.switch(self.table.range_for(low));
        }
        self.current
    }
}

/// Whether `hz` is within [`HARMONIC_TOLERANCE_CENTS`] of a whole multiple, two or more, of
/// `fundamental`.
fn harmonic(hz: f64, fundamental: f64) -> bool {
    let ratio = hz / fundamental;
    let k = ratio.round();
    k >= 2.0 && (1_200.0 * (ratio / k).log2()).abs() <= HARMONIC_TOLERANCE_CENTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_derived_from_the_rate() {
        let shapes = |rate| {
            let table = RangeTable::new(rate).unwrap();
            Range::ALL.map(|range| {
                let spec = table.spec(range);
                (spec.analysis_rate, spec.window, spec.hop)
            })
        };
        assert_eq!(
            shapes(48_000),
            [(12_000, 1_200, 60), (12_000, 480, 30), (48_000, 900, 120)]
        );
        // 3 · 11025 / 30 = 1102.5 and 3 · 44100 / 160 = 826.9, rounded up; hops rounded.
        assert_eq!(
            shapes(44_100),
            [(11_025, 1_103, 55), (11_025, 441, 28), (44_100, 827, 110)]
        );
        for rate in [0, 8_000, 22_050, 88_200, 96_000] {
            assert_eq!(RangeTable::new(rate), Err(FilterError::SampleRate(rate)));
        }
    }

    #[test]
    fn every_window_holds_three_periods_of_its_floor_and_its_lowest_held_note() {
        for rate in SUPPORTED_RATES {
            let table = RangeTable::new(rate).unwrap();
            for range in Range::ALL {
                let spec = table.spec(range);
                let period = f64::from(spec.analysis_rate) / range.floor_hz();
                let periods = index_to_f64(spec.window) / period;
                assert!(
                    (3.0..3.01).contains(&periods),
                    "{range:?} at {rate}: {periods}"
                );
                // The longest period a range holds must be a lag `Mpm` searches (up to N/2), with
                // a neighbour on each side for the parabola.
                let lowest = range.floor_hz()
                    / if range == Range::Low {
                        30.0 / 25.0
                    } else {
                        HYSTERESIS
                    };
                let longest = f64::from(spec.analysis_rate) / lowest;
                assert!(
                    longest + 1.0 < index_to_f64(spec.window / 2),
                    "{range:?} at {rate}"
                );
            }
        }
    }

    #[test]
    fn the_bands_overlap_by_the_hysteresis() {
        let table = RangeTable::new(48_000).unwrap();
        assert_eq!(table.range_for(20.0), Range::Low);
        assert_eq!(table.range_for(74.99), Range::Low);
        assert_eq!(table.range_for(75.0), Range::Mid);
        assert_eq!(table.range_for(159.99), Range::Mid);
        assert_eq!(table.range_for(160.0), Range::High);
        assert_eq!(table.range_for(f64::NAN), Range::Low);
        let edge = |range, hz| table.holds(range, hz);
        assert!(edge(Range::Low, 79.4) && !edge(Range::Low, 79.5));
        assert!(edge(Range::Mid, 70.8) && !edge(Range::Mid, 70.7));
        assert!(edge(Range::Mid, 169.5) && !edge(Range::Mid, 169.6));
        assert!(edge(Range::High, 151.1) && !edge(Range::High, 150.9));
        assert!(edge(Range::High, 5_000.0) && !edge(Range::High, f64::INFINITY));
    }

    #[test]
    fn a_pitch_outside_the_current_range_moves_to_its_own() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Low);
        assert_eq!(selector.observe(Observation::Pitch(77.0)), Range::Low); // hysteresis
        assert_eq!(selector.observe(Observation::Pitch(80.0)), Range::Mid);
        assert_eq!(selector.observe(Observation::Pitch(165.0)), Range::Mid);
        assert_eq!(selector.observe(Observation::Pitch(170.0)), Range::High);
        assert_eq!(selector.observe(Observation::Pitch(152.0)), Range::High);
        assert_eq!(selector.observe(Observation::Pitch(150.0)), Range::Mid);
        assert_eq!(selector.observe(Observation::Pitch(41.2)), Range::Low);
        assert_eq!(selector.observe(Observation::Pitch(1_000.0)), Range::High);
    }

    #[test]
    fn silence_returns_home_and_unclear_frames_scan_after_a_window_and_a_grace() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Mid);
        selector.observe(Observation::Pitch(330.0));
        assert_eq!(selector.observe(Observation::Silent), Range::Mid);

        // Mid: (40 ms + 50 ms) / 2.5 ms = 36 frames.
        assert_eq!(selector.miss_limit(Range::Mid), 36);
        assert_eq!(selector.miss_limit(Range::Low), 30);
        assert_eq!(selector.miss_limit(Range::High), 28);
        for _ in 0..35 {
            assert_eq!(selector.observe(Observation::Unclear), Range::Mid);
        }
        assert_eq!(selector.observe(Observation::Unclear), Range::High);
        // A nonsensical pitch counts as unclear; a clear one takes one miss away (27 − 1 = 26),
        // so two more unclear frames reach High's limit of 28.
        for _ in 0..27 {
            selector.observe(Observation::Pitch(f64::NAN));
        }
        assert_eq!(selector.observe(Observation::Pitch(400.0)), Range::High);
        assert_eq!(selector.observe(Observation::Unclear), Range::High);
        assert_eq!(selector.observe(Observation::Unclear), Range::Low);
    }

    #[test]
    fn a_clear_low_reference_below_the_band_catches_the_octave_above() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::High);
        selector.observe(Observation::Pitch(165.35));
        // Not a harmonic of the reference: a melody over a bass note keeps its range.
        assert_eq!(selector.observe_reference(Some(61.0)), Range::High);
        // Inside the current band, or unclear: nothing to act on.
        assert_eq!(selector.observe_reference(Some(160.0)), Range::High);
        assert_eq!(selector.observe_reference(None), Range::High);
        // Twice the reference: the octave above, caught.
        assert_eq!(selector.observe_reference(Some(82.41)), Range::Mid);
        // From Mid, a bass E read as its second harmonic; and with no clear pitch at all.
        selector.observe(Observation::Pitch(82.4));
        assert_eq!(selector.observe_reference(Some(41.2)), Range::Low);
        let mut unclear = RangeSelector::new(&table, Range::Mid);
        unclear.observe(Observation::Unclear);
        assert_eq!(unclear.observe_reference(Some(41.2)), Range::Low);
        // In Low itself the reference is the current range: nothing to do.
        assert_eq!(unclear.observe_reference(Some(30.0)), Range::Low);
    }

    #[test]
    fn clear_frames_decay_the_misses_rather_than_reset_them() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Mid);
        // Two unclear frames for every clear one still reach Mid's limit of 36: after 34 cycles
        // of (unclear, unclear, clear) and two more unclear frames, 104 in all. A reset would
        // never get past 2.
        let mut frames = 0;
        while selector.current() == Range::Mid && frames < 200 {
            let observation = if frames % 3 == 2 {
                Observation::Pitch(82.41)
            } else {
                Observation::Unclear
            };
            selector.observe(observation);
            frames += 1;
        }
        assert_eq!((selector.current(), frames), (Range::High, 104));
    }

    #[test]
    fn harmonics_are_whole_multiples_within_half_a_semitone() {
        assert!(harmonic(165.35, 82.41) && harmonic(330.0, 82.41) && harmonic(82.0, 41.2));
        assert!(!harmonic(82.41, 82.41) && !harmonic(165.35, 61.0) && !harmonic(120.0, 82.41));
    }

    /// The ledger table in `docs/AUDIO_ENGINE.md` §1, at 48 kHz, in ms: each range's chain line and
    /// total. Changing a constant it is built from (`IO_SECONDS`, a settling, a hop, a filter)
    /// fails this until the table is updated to match.
    #[test]
    fn the_ledger_matches_the_table_in_audio_engine_section_1() {
        let table = RangeTable::new(48_000).unwrap();
        let ms = |range| {
            let ledger = table.ledger(range);
            let round = |s: f64| (s * 1e4).round() / 10.0;
            (round(ledger.chain_seconds()), round(ledger.total_seconds()))
        };
        assert_eq!(ms(Range::Low), (122.7, 186.0));
        assert_eq!(ms(Range::Mid), (50.2, 108.5));
        assert_eq!(ms(Range::High), (21.9, 80.2));
        // §1's budgets, by range: High ≤ 85 ms (f₀ ≥ 160 Hz), Mid ≤ 110 ms (80–160 Hz), Low
        // ≤ 190 ms (low B), at both rates.
        for rate in SUPPORTED_RATES {
            let table = RangeTable::new(rate).unwrap();
            for (range, budget) in [
                (Range::Low, 0.190),
                (Range::Mid, 0.110),
                (Range::High, 0.085),
            ] {
                let total = table.ledger(range).total_seconds();
                assert!(total <= budget, "{range:?} at {rate} Hz: {total} s");
            }
        }
    }

    #[test]
    fn the_ledger_adds_up() {
        let table = RangeTable::new(48_000).unwrap();
        let low = table.ledger(Range::Low);
        let expected_chain = low.group_delay + 0.1 + 0.015 + 0.005;
        assert!((low.chain_seconds() - expected_chain).abs() < 1e-12);
        let fixed = IO_SECONDS + SNAPSHOT_SECONDS + DISPLAY_SECONDS;
        assert!((low.total_seconds() - (expected_chain + 0.01 + fixed)).abs() < 1e-12);
    }
}

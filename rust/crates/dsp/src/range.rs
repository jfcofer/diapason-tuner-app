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

    /// The perceived-latency budget of the notes this range serves, in seconds:
    /// `docs/AUDIO_ENGINE.md` §1's rows, as the owner set them on 2026-10-23 (`docs/adr/0028`).
    /// High's is the row for f₀ ≥ 160 Hz, Mid's for 80–160 Hz, Low's for low B. A test reads them
    /// back from §1.
    #[must_use]
    pub fn latency_budget_seconds(self) -> f64 {
        match self {
            Self::Low => 0.190,
            Self::Mid => 0.110,
            Self::High => 0.085,
        }
    }

    /// The next range in a scan: downward, wrapping from the lowest to the highest. A range that
    /// hears no pitch is most often below its reach (High is blind under 107 Hz, Mid under 50 Hz),
    /// while Low, which sees down to 20 Hz, misses only what is above its 1.5 kHz content. An
    /// upward order took a downward octave from High to Low and back up to Mid: two switches.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Low => Self::High,
            Self::Mid => Self::Low,
            Self::High => Self::Mid,
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
    /// [`HYSTERESIS`] where that cannot cross a latency row of `docs/AUDIO_ENGINE.md` §1. Low holds
    /// up to 79.46 Hz, Mid from 70.79 Hz up to 160 Hz exclusive, High from 151.0 Hz: every
    /// f₀ ≥ 80 Hz is out of Low, and every f₀ ≥ 160 Hz is High.
    #[must_use]
    pub fn holds(&self, range: Range, hz: f64) -> bool {
        let above_floor = range == Range::Low || hz >= range.floor_hz() / HYSTERESIS;
        let below_ceiling = match range {
            // Low's band reaches a semitone past Mid's floor, to 79.46 Hz: still under the 80 Hz
            // where §1's 110 ms row starts.
            Range::Low => hz < Range::Mid.floor_hz() * HYSTERESIS,
            // Mid stops at the crossover: §1's 85 ms row starts at 160 Hz, so the overlap with
            // High sits below it, where High holds down to 151 Hz.
            Range::Mid => hz < Range::High.floor_hz(),
            Range::High => true,
        };
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

/// The highest multiple [`RangeSelector::observe_reference`] takes for a harmonic. A weak
/// fundamental leaves the second or third partial strongest, so a range blind to the fundamental
/// reads 2·f₀ or 3·f₀; 4 leaves one partial of room. Unbounded, ±50 ¢ makes every frequency above
/// 17·f₀ "a multiple" (587.33 Hz is 14 × 41.2 Hz + 31 ¢), and a melody over a bass note would
/// fall to the bass's range.
pub const MAX_HARMONIC: f64 = 4.0;

/// A change of the current range's clear pitch larger than this, in cents, makes every Low frame
/// whose window started before it stale for [`RangeSelector::observe_reference`]. Three quarters
/// of a semitone: any change of note, but not the jitter of a weak, barely clear reading (Mid's
/// reading of a weak bass E wanders by 53 ¢ between frames). It compares each clear frame with the
/// previous clear one, so a slow glide (a bend, a peg turned) is never a change, however far it
/// goes.
pub const CHANGE_CENTS: f64 = 75.0;

/// How long, in seconds, a streak of agreeing Low reference frames below the band, each consistent
/// with the current pitch, moves the chain even when none of them is fresh. Freshness can be
/// starved: a range whose wrong reading breaks every few frames, or flips between two partials,
/// keeps restarting it (`T-003b` round 4: stuck in High for 10 s). 250 ms is more than twice Low's
/// span (105 ms), so a streak that long cannot be made only of Low windows that span a note change.
pub const REFERENCE_PATIENCE_SECONDS: f64 = 0.25;

/// Chooses the range for the next frame from what the last one found. A pure state machine: its
/// only clock is the stream position each frame reports, and it does no I/O and no allocation.
///
/// - A clear pitch the current range [`holds`](RangeTable::holds) keeps it. One above its band
///   moves at once to [`RangeTable::range_for`] the pitch. One below moves only after it has held
///   (within [`CHANGE_CENTS`]) for a whole window of the current range: a window spanning a note
///   change finds the two notes' common period, lower than either. The bands overlap, so a note
///   tuned across a boundary stays where it is.
/// - Silence (a closed gate) returns to the home range, the one a preset picks (M3).
/// - Each frame without a clear pitch, gate open, adds a miss; each clear one takes a miss away
///   (a decay, not a reset, so sporadic clear frames cannot hold a range that mostly sees
///   nothing). After a window and [`MISS_GRACE_SECONDS`] of misses it tries the next range
///   ([`Range::next`]). That is how a note no range can see from home (1.4 kHz is in the
///   decimated path's stopband) is found: each range's history is kept full, so a switch costs
///   one hop, not a window.
/// - **The octave guard of range selection.** `Mpm` searches lags up to half its window, so a
///   range cannot see below about 1.5 periods of its floor: High is blind below 107 Hz, Mid below
///   50 Hz. A low E (82.41 Hz) heard by High reads as its second partial, 164.8 Hz, which High
///   holds. So the Low range, which sees down to 20 Hz, is analysed as a reference while another
///   range is selected ([`Self::observe_reference`]). It moves the chain to the Low pitch's range
///   only on evidence that cannot come from a note change:
///   - the Low pitch is below the current band, and the current range's latest clear pitch since
///     the last switch is absent, or 2, 3 or 4 times it ([`MAX_HARMONIC`]);
///   - **the Low frame's whole window, filter taps included, started after the last switch, the
///     last change of the current pitch ([`CHANGE_CENTS`]) and the last end of a reading held
///     for a window ([`Self::held_frames`])**. A Low window that spans a note
///     change holds both notes, and its NSDF finds their common period, f/2, f/3, …: acting on
///     it showed wrong notes for up to 55 ms, and flipped Mid and High on every frame after an
///     upward leap (`T-003b`);
///   - two such Low frames in a row agree within [`HARMONIC_TOLERANCE_CENTS`];
///   - or, whatever freshness says, such Low frames have agreed for
///     [`REFERENCE_PATIENCE_SECONDS`] on end: a bound on how long a wrong reading can hold a range.
///
/// ```
/// use diapason_dsp::range::{Observation, Range, RangeSelector, RangeTable};
///
/// let table = RangeTable::new(48_000)?;
/// let mut selector = RangeSelector::new(&table, Range::Low);
/// // Positions are input samples, the audio clock: one frame every 2.5 ms here.
/// let mut at = 0;
/// let mut next = || { at += 120; at };
/// assert_eq!(selector.observe(Observation::Pitch(82.41), next()), Range::Mid);
/// assert_eq!(selector.observe(Observation::Pitch(72.0), next()), Range::Mid); // hysteresis
/// assert_eq!(selector.observe(Observation::Pitch(330.0), next()), Range::High);
/// assert_eq!(selector.observe(Observation::Pitch(164.82), next()), Range::High); // octave up
/// // A Low frame whose window still holds the E4 is stale; 120 ms later, two agree.
/// assert_eq!(selector.observe_reference(Some(82.41), next()), Range::High);
/// let later = at + 5_760;
/// assert_eq!(selector.observe_reference(Some(82.41), later), Range::High);
/// assert_eq!(selector.observe_reference(Some(82.4), later + 240), Range::Mid);
/// assert_eq!(selector.observe(Observation::Silent, later + 480), Range::Low);
/// # Ok::<(), diapason_dsp::filter::FilterError>(())
/// ```
#[derive(Debug, Clone)]
pub struct RangeSelector {
    table: RangeTable,
    home: Range,
    current: Range,
    misses: usize,
    /// The current range's latest clear pitch since the last switch.
    pitch: Option<f64>,
    /// The stream position of the last switch, return home, or change of `pitch`.
    settled_at: u64,
    /// The last eligible Low reference pitch, for the agreement of two in a row.
    reference: Option<f64>,
    /// Consecutive clear frames in the current range.
    clear_run: usize,
    /// When the current streak of agreeing candidate Low references began, and its latest pitch.
    streak: Option<(u64, f64)>,
    /// [`REFERENCE_PATIENCE_SECONDS`] in input samples.
    patience: u64,
    /// Input samples a frame of each range depends on, filter taps included.
    spans: [u64; 3],
}

impl RangeSelector {
    /// A selector over `table`, starting at, and returning to, `home`.
    #[must_use]
    pub fn new(table: &RangeTable, home: Range) -> Self {
        let spans = Range::ALL.map(|range| {
            let spec = table.spec(range);
            let factor = match spec.path {
                Path::Decimated => DECIMATION,
                Path::Full => 1,
            };
            let span = spec.window * factor + 2 * table.group_delay(spec.path) + 1;
            u64::try_from(span).unwrap_or(u64::MAX)
        });
        Self {
            table: *table,
            home,
            current: home,
            misses: 0,
            pitch: None,
            settled_at: 0,
            reference: None,
            clear_run: 0,
            streak: None,
            patience: u64::try_from(index_from_f64(
                (REFERENCE_PATIENCE_SECONDS * f64::from(table.sample_rate())).round(),
            ))
            .unwrap_or(u64::MAX),
            spans,
        }
    }

    /// Clear frames covering one window of `range`. A run that long, ended by an unclear frame, is
    /// a held note ending: a change, like a new pitch (see [`CHANGE_CENTS`]). Shorter runs are
    /// not: a range's barely clear reading of a weak fundamental's partial (High's of a low E at
    /// clarity 0.6) comes and goes in runs of a few frames.
    #[must_use]
    pub fn held_frames(&self, range: Range) -> usize {
        let spec = self.table.spec(range);
        spec.window.div_ceil(spec.hop.max(1))
    }

    /// Whether a frame of `range` that ended at `end` saw nothing from before `settled_at`.
    fn fresh(&self, range: Range, end: u64) -> bool {
        let [low, mid, high] = self.spans;
        let span = match range {
            Range::Low => low,
            Range::Mid => mid,
            Range::High => high,
        };
        end.saturating_sub(span) >= self.settled_at
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

    fn switch(&mut self, range: Range, at: u64) {
        if range != self.current {
            self.current = range;
            self.misses = 0;
            self.pitch = None;
            self.settled_at = at;
            self.reference = None;
            self.clear_run = 0;
            self.streak = None;
        }
    }

    /// A position earlier than the state's own (the stream restarted without a new selector):
    /// forget the timing, so that nothing waits for a moment that already passed.
    fn rewind(&mut self, end: u64) {
        if end < self.settled_at || self.streak.is_some_and(|(start, _)| end < start) {
            self.settled_at = end;
            self.streak = None;
        }
    }

    /// Take the outcome of the frame that ended at stream position `end`; return the range for the
    /// next frame.
    pub fn observe(&mut self, observation: Observation, end: u64) -> Range {
        self.rewind(end);
        match observation {
            Observation::Pitch(hz) if hz.is_finite() && hz > 0.0 => {
                self.misses = self.misses.saturating_sub(1);
                self.clear_run += 1;
                if self
                    .pitch
                    .is_none_or(|old| cents(hz, old).abs() > CHANGE_CENTS)
                {
                    self.settled_at = end;
                    self.reference = None;
                }
                self.pitch = Some(hz);
                // Upward at once. Downward only once the reading has held for a whole window: a
                // window that spans a note change finds the notes' common period, which is lower
                // than either, and would otherwise drop the chain into a range showing it.
                let upward = hz >= self.current.floor_hz();
                if !self.table.holds(self.current, hz) && (upward || self.fresh(self.current, end))
                {
                    self.switch(self.table.range_for(hz), end);
                }
            }
            Observation::Silent => {
                self.misses = 0;
                self.pitch = None;
                self.current = self.home;
                self.settled_at = end;
                self.reference = None;
                self.clear_run = 0;
            }
            Observation::Pitch(_) | Observation::Unclear => {
                if self.clear_run >= self.held_frames(self.current) {
                    self.settled_at = end;
                    self.reference = None;
                }
                self.clear_run = 0;
                self.misses += 1;
                if self.misses >= self.miss_limit(self.current) {
                    self.switch(self.current.next(), end);
                    self.misses = 0;
                }
            }
        }
        self.current
    }

    /// Take a frame of the Low range that ended at stream position `end`, analysed as a reference
    /// while another range is selected; return the range for the next frame. See the type's notes
    /// for what it takes to act on it.
    pub fn observe_reference(&mut self, low: Option<f64>, end: u64) -> Range {
        self.rewind(end);
        let candidate = low.filter(|&hz| {
            let below = hz < self.current.floor_hz() && !self.table.holds(self.current, hz);
            let consistent = self.pitch.is_none_or(|current| harmonic(current, hz));
            hz.is_finite() && hz > 0.0 && below && consistent
        });
        let Some(hz) = candidate.filter(|_| self.current != Range::Low) else {
            self.reference = None;
            self.streak = None;
            return self.current;
        };
        let agrees = |previous: f64| cents(hz, previous).abs() <= HARMONIC_TOLERANCE_CENTS;
        let start = match self.streak {
            Some((start, previous)) if agrees(previous) => start,
            _ => end,
        };
        self.streak = Some((start, hz));
        let patient = end.saturating_sub(start) >= self.patience;
        if !self.fresh(Range::Low, end) {
            self.reference = None;
            if patient {
                self.switch(self.table.range_for(hz), end);
            }
        } else if self.reference.is_some_and(agrees) || patient {
            self.switch(self.table.range_for(hz), end);
        } else {
            self.reference = Some(hz);
        }
        self.current
    }
}

/// `1200 · log₂(a / b)`.
fn cents(a: f64, b: f64) -> f64 {
    1_200.0 * (a / b).log2()
}

/// Whether `hz` is within [`HARMONIC_TOLERANCE_CENTS`] of 2, 3 or 4 ([`MAX_HARMONIC`]) times
/// `fundamental`.
fn harmonic(hz: f64, fundamental: f64) -> bool {
    let ratio = hz / fundamental;
    let k = ratio.round();
    (2.0..=MAX_HARMONIC).contains(&k) && cents(ratio, k).abs() <= HARMONIC_TOLERANCE_CENTS
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
    fn the_bands_overlap_below_each_budget_boundary() {
        let table = RangeTable::new(48_000).unwrap();
        assert_eq!(table.range_for(20.0), Range::Low);
        assert_eq!(table.range_for(74.99), Range::Low);
        assert_eq!(table.range_for(75.0), Range::Mid);
        assert_eq!(table.range_for(159.99), Range::Mid);
        assert_eq!(table.range_for(160.0), Range::High);
        assert_eq!(table.range_for(f64::NAN), Range::Low);
        let edge = |range, hz| table.holds(range, hz);
        // Low reaches past 75 Hz but stops short of §1's 80 Hz row; Mid stops at 160 Hz, where the
        // 85 ms row starts, and High reaches down to 151 Hz instead.
        assert!(edge(Range::Low, 79.4) && !edge(Range::Low, 79.5));
        assert!(edge(Range::Mid, 70.8) && !edge(Range::Mid, 70.7));
        assert!(edge(Range::Mid, 159.99) && !edge(Range::Mid, 160.0));
        assert!(edge(Range::High, 151.1) && !edge(Range::High, 150.9));
        assert!(edge(Range::High, 5_000.0) && !edge(Range::High, f64::INFINITY));
    }

    /// Stream positions for frames 2.5 ms apart at 48 kHz, the hop of Mid and High.
    struct Clock(u64);

    impl Clock {
        fn tick(&mut self) -> u64 {
            self.0 += 120;
            self.0
        }
    }

    #[test]
    fn a_pitch_above_the_band_moves_at_once_and_one_below_once_it_has_held_a_window() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Low);
        let mut clock = Clock(0);
        assert_eq!(
            selector.observe(Observation::Pitch(77.0), clock.tick()),
            Range::Low
        );
        assert_eq!(
            selector.observe(Observation::Pitch(80.0), clock.tick()),
            Range::Mid
        );
        assert_eq!(
            selector.observe(Observation::Pitch(159.0), clock.tick()),
            Range::Mid
        );
        assert_eq!(
            selector.observe(Observation::Pitch(160.0), clock.tick()),
            Range::High
        );
        assert_eq!(
            selector.observe(Observation::Pitch(152.0), clock.tick()),
            Range::High
        );
        // A single low reading, a window's common period across a note change, is not enough.
        selector.observe(Observation::Pitch(196.0), clock.tick());
        assert_eq!(
            selector.observe(Observation::Pitch(65.4), clock.tick()),
            Range::High
        );
        assert_eq!(
            selector.observe(Observation::Pitch(196.0), clock.tick()),
            Range::High
        );
        // A reading below the band that holds for High's window (959 samples, 8 frames) moves.
        let start = clock.tick();
        assert_eq!(
            selector.observe(Observation::Pitch(110.0), start),
            Range::High
        );
        for _ in 0..7 {
            assert_eq!(
                selector.observe(Observation::Pitch(110.1), clock.tick()),
                Range::High
            );
        }
        assert_eq!(
            selector.observe(Observation::Pitch(110.0), clock.tick()),
            Range::Mid
        );
    }

    #[test]
    fn silence_returns_home_and_unclear_frames_scan_after_a_window_and_a_grace() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Mid);
        let mut clock = Clock(0);
        selector.observe(Observation::Pitch(330.0), clock.tick());
        assert_eq!(
            selector.observe(Observation::Silent, clock.tick()),
            Range::Mid
        );

        // Mid: (40 ms + 50 ms) / 2.5 ms = 36 frames.
        assert_eq!(selector.miss_limit(Range::Mid), 36);
        assert_eq!(selector.miss_limit(Range::Low), 30);
        assert_eq!(selector.miss_limit(Range::High), 28);
        for _ in 0..35 {
            assert_eq!(
                selector.observe(Observation::Unclear, clock.tick()),
                Range::Mid
            );
        }
        // Downward: Mid gives way to Low, and Low, the lowest, to High.
        assert_eq!(
            selector.observe(Observation::Unclear, clock.tick()),
            Range::Low
        );
        for _ in 0..29 {
            assert_eq!(
                selector.observe(Observation::Unclear, clock.tick()),
                Range::Low
            );
        }
        assert_eq!(
            selector.observe(Observation::Unclear, clock.tick()),
            Range::High
        );
        // A nonsensical pitch counts as unclear; a clear one takes one miss away (27 − 1 = 26),
        // so two more unclear frames reach High's limit of 28.
        for _ in 0..27 {
            selector.observe(Observation::Pitch(f64::NAN), clock.tick());
        }
        assert_eq!(
            selector.observe(Observation::Pitch(400.0), clock.tick()),
            Range::High
        );
        assert_eq!(
            selector.observe(Observation::Unclear, clock.tick()),
            Range::High
        );
        assert_eq!(
            selector.observe(Observation::Unclear, clock.tick()),
            Range::Mid
        );
    }

    /// Low's span at 48 kHz: 1200 decimated samples × 4 plus the 259 taps.
    const LOW_SPAN: u64 = 5_059;

    #[test]
    fn a_fresh_agreeing_low_reference_below_the_band_catches_the_octave_above() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::High);
        let changed = 10_000;
        selector.observe(Observation::Pitch(165.35), changed);
        // A Low window that started before the current pitch began is stale, however clear.
        for end in [changed + 120, changed + LOW_SPAN - 1] {
            assert_eq!(selector.observe_reference(Some(82.41), end), Range::High);
        }
        // Fresh, but not a harmonic, or inside the band, or unclear: nothing to act on.
        let fresh = changed + LOW_SPAN;
        assert_eq!(selector.observe_reference(Some(61.0), fresh), Range::High);
        assert_eq!(
            selector.observe_reference(Some(160.0), fresh + 240),
            Range::High
        );
        assert_eq!(selector.observe_reference(None, fresh + 480), Range::High);
        // Fresh and twice the reference: acted on once a second Low frame agrees.
        assert_eq!(
            selector.observe_reference(Some(82.41), fresh + 720),
            Range::High
        );
        assert_eq!(
            selector.observe_reference(Some(82.45), fresh + 960),
            Range::Mid
        );
    }

    #[test]
    fn one_unclear_frame_is_not_a_missing_pitch_but_a_held_reading_ending_is_a_change() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Mid);
        let mut clock = Clock(0);
        // Mid holds 130.8 Hz for a second, then goes unclear for a frame: the pitch stays 130.8,
        // so a Low common period of 61 Hz, not a multiple of it, changes nothing.
        for _ in 0..400 {
            selector.observe(Observation::Pitch(130.81), clock.tick());
        }
        let ended = clock.tick();
        selector.observe(Observation::Unclear, ended);
        assert_eq!(
            selector.observe_reference(Some(61.0), ended + 120),
            Range::Mid
        );
        // And 65.4 Hz, half of it, is now stale: the held reading ended at `ended`, so only Low
        // frames whose window started after it count (the C3 → G3 leap's common period).
        assert_eq!(
            selector.observe_reference(Some(65.4), ended + 120),
            Range::Mid
        );
        assert_eq!(
            selector.observe_reference(Some(65.4), ended + 240),
            Range::Mid
        );
        // A short clear run (a barely clear reading) ending is not a change.
        let mut weak = RangeSelector::new(&table, Range::High);
        let mut clock = Clock(0);
        let first = clock.tick();
        weak.observe(Observation::Pitch(164.8), first);
        weak.observe(Observation::Unclear, clock.tick());
        weak.observe(Observation::Pitch(164.9), clock.tick());
        weak.observe(Observation::Unclear, clock.tick());
        let fresh = first + LOW_SPAN;
        assert_eq!(weak.observe_reference(Some(82.41), fresh), Range::High);
        assert_eq!(weak.observe_reference(Some(82.41), fresh + 240), Range::Mid);
        assert_eq!(selector.held_frames(Range::High), 8);
    }

    /// Frames every 2.5 ms at 48 kHz, a Low reference every other one (Low's 5 ms hop), and the
    /// frame index at which the selector left High; `None` if it never did in 10 s.
    fn leaves_high(current: impl Fn(usize) -> Observation) -> Option<usize> {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::High);
        let mut clock = Clock(0);
        (0..4_000).find(|&frame| {
            let end = clock.tick();
            selector.observe(current(frame), end);
            if frame % 2 == 1 {
                selector.observe_reference(Some(82.41), end);
            }
            selector.current() != Range::High
        })
    }

    /// The bound, in frames: the first reference (frame 1, at 240 samples) plus
    /// `REFERENCE_PATIENCE_SECONDS` (12 000 samples at 48 kHz) is frame 101, 252.5 ms in.
    const PATIENCE_BOUND: usize = 101;

    /// Pattern A (`T-003b` round 4): High's wrong reading of a low E, 164.8 Hz, in clear runs of
    /// nine broken by one unclear frame. Every break ends a held reading and restarts freshness, so
    /// the fresh path never fires; the agreeing streak of Low references does, within the bound.
    #[test]
    fn a_reading_broken_every_few_frames_cannot_hold_a_range_past_the_patience() {
        let left = leaves_high(|frame| {
            if frame % 10 == 9 {
                Observation::Unclear
            } else {
                Observation::Pitch(164.8)
            }
        });
        assert_eq!(left, Some(PATIENCE_BOUND));
    }

    /// Pattern B: High flips between the second and third partials, 164.8 and 247.2 Hz, every
    /// 75 ms. Each flip is a change of pitch that restarts freshness; both are multiples of the
    /// Low reference, so the streak holds, and the selector leaves within the bound.
    #[test]
    fn a_reading_flipping_between_partials_cannot_hold_a_range_past_the_patience() {
        let left = leaves_high(|frame| {
            Observation::Pitch(if (frame / 30) % 2 == 0 { 164.8 } else { 247.2 })
        });
        assert_eq!(left, Some(PATIENCE_BOUND));
    }

    /// A position from before the selector's last change (the stream restarted) is not a moment
    /// to wait for: the selector rewinds, and the fresh path works again.
    #[test]
    fn a_stream_position_going_backwards_does_not_freeze_the_selector() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::High);
        selector.observe(Observation::Pitch(164.8), 1_000_000_000);
        let mut clock = Clock(0);
        let left = (0..4_000).find(|&frame| {
            let end = clock.tick();
            selector.observe(Observation::Pitch(164.8), end);
            if frame % 2 == 1 {
                selector.observe_reference(Some(82.41), end);
            }
            selector.current() != Range::High
        });
        // Fresh once Low's span (5059 samples) has passed since the rewind, then two agree.
        assert!(left.is_some_and(|frame| frame < 50), "{left:?}");
    }

    #[test]
    fn clear_frames_decay_the_misses_rather_than_reset_them() {
        let table = RangeTable::new(48_000).unwrap();
        let mut selector = RangeSelector::new(&table, Range::Mid);
        let mut clock = Clock(0);
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
            selector.observe(observation, clock.tick());
            frames += 1;
        }
        assert_eq!((selector.current(), frames), (Range::Low, 104));
    }

    #[test]
    fn harmonics_are_two_to_four_times_within_half_a_semitone() {
        assert!(harmonic(165.35, 82.41) && harmonic(247.2, 82.41) && harmonic(330.0, 82.41));
        assert!(!harmonic(82.41, 82.41) && !harmonic(165.35, 61.0) && !harmonic(120.0, 82.41));
        // Beyond the fourth, every frequency would be "a multiple": D5 is 14 × 41.2 Hz + 31 ¢.
        assert!(!harmonic(412.0, 82.41) && !harmonic(587.33, 41.2));
    }

    /// `docs/AUDIO_ENGINE.md`, read at compile time, so that §1 and the code cannot drift apart.
    const AUDIO_ENGINE: &str = include_str!("../../../../docs/AUDIO_ENGINE.md");

    /// The first number in a table cell, ignoring emphasis and anything in parentheses after it,
    /// and the rounding its printed digits imply: half the last digit.
    fn cell_number(cell: &str) -> (f64, f64) {
        let text: String = cell
            .trim()
            .trim_start_matches(['*', '≤', ' '])
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let decimals = text.split('.').nth(1).map_or(0, str::len);
        let rounding = 0.5 * 10_f64.powi(-i32::try_from(decimals).unwrap()) + 1e-9;
        (
            text.parse()
                .unwrap_or_else(|_| panic!("no number in {cell:?}")),
            rounding,
        )
    }

    /// §1's ledger table, read from the doc: every row, every range, against `Ledger` at 48 kHz,
    /// to the precision printed. A row renamed, added or dropped fails too.
    #[test]
    fn the_ledger_table_in_audio_engine_section_1_matches_the_code() {
        type Line = fn(&Ledger) -> f64;
        let table = RangeTable::new(48_000).unwrap();
        let start = AUDIO_ENGINE
            .find("| Stage | Low")
            .expect("§1's ledger table");
        let rows: Vec<Vec<&str>> = AUDIO_ENGINE[start..]
            .lines()
            .skip(2)
            .take_while(|line| line.starts_with('|'))
            .map(|line| line.trim_matches('|').split('|').collect())
            .collect();
        let lines: [(&str, Line); 10] = [
            ("Input I/O", |l| l.io),
            ("Filter group delay", |l| l.group_delay),
            ("Window fill", |l| l.window),
            ("Settling", |l| l.settling),
            ("One hop", |l| l.hop),
            ("**Chain line", Ledger::chain_seconds),
            ("Smoothing reserve", |l| l.smoothing),
            ("Snapshot period", |l| l.snapshot),
            ("One display frame", |l| l.display),
            ("**Total**", Ledger::total_seconds),
        ];
        assert_eq!(
            rows.len(),
            lines.len(),
            "§1's ledger has {} rows",
            rows.len()
        );
        for row in &rows {
            assert_eq!(
                row.len(),
                4,
                "§1's row {:?} needs a label and three ranges",
                row[0]
            );
        }
        for (row, (label, line)) in rows.iter().zip(lines) {
            assert!(
                row[0].trim().starts_with(label),
                "expected {label:?}, found {:?}",
                row[0]
            );
            for (cell, range) in row[1..].iter().zip(Range::ALL) {
                let (printed, rounding) = cell_number(cell);
                let code = line(&table.ledger(range)) * 1e3;
                assert!(
                    (printed - code).abs() <= rounding,
                    "§1 {label} for {range:?}: {printed} ms in the doc, {code:.3} ms in the code"
                );
            }
        }
        // The totals row names each range's budget in parentheses; §1's budget rows state them.
        let totals = rows.last().unwrap();
        let rows_for = ["f₀ ≥ 160 Hz", "Same, 80–160 Hz", "Same, bass low B"];
        for (range, budget_row) in [Range::High, Range::Mid, Range::Low]
            .into_iter()
            .zip(rows_for)
        {
            let budget = range.latency_budget_seconds() * 1e3;
            let cell = totals[1 + Range::ALL.iter().position(|r| *r == range).unwrap()];
            let in_totals = cell.split('(').nth(1).map(cell_number).unwrap().0;
            let line = AUDIO_ENGINE
                .lines()
                .find(|l| l.contains(budget_row))
                .unwrap();
            let in_budgets = cell_number(line.split('|').nth(2).unwrap()).0;
            assert!(
                (in_totals - budget).abs() < 1e-9 && (in_budgets - budget).abs() < 1e-9,
                "{range:?}: budget {budget} ms in the code, {in_totals} and {in_budgets} in §1"
            );
        }
    }

    /// Every range's total, at both rates, within its budget.
    #[test]
    fn every_total_is_within_its_budget() {
        for rate in SUPPORTED_RATES {
            let table = RangeTable::new(rate).unwrap();
            for range in Range::ALL {
                let total = table.ledger(range).total_seconds();
                let budget = range.latency_budget_seconds();
                assert!(
                    total <= budget,
                    "{range:?} at {rate} Hz: {total} s over {budget} s"
                );
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

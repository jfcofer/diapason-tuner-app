//! The adaptive noise gate (`docs/AUDIO_ENGINE.md` §2): is anything being played?
//!
//! The gate *reports*; it never mutes. Zeroing the samples the detector sees would cut the onset of
//! every note and change the frame's shape, so the analysis always gets the audio, and the gate's
//! state decides only whether an estimate is shown ("listening" otherwise).

use std::fmt;

use crate::convert::{index_from_f64, to_f32};
use crate::level::RmsMeter;

/// The gate's parameters. [`GateConfig::default`] holds the documented values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateConfig {
    /// Length of one RMS block, in seconds. Decisions are made once per block.
    pub block_seconds: f64,
    /// How much learned history the noise floor is the minimum of, in seconds: blocks heard while
    /// the gate was closed, or open and unpitched ([`NoiseGate::set_unpitched`]).
    pub floor_seconds: f64,
    /// The open threshold never goes below this, in dBFS.
    pub minimum_open_dbfs: f64,
    /// The open threshold never goes above this, in dBFS, so that a learned floor, however high,
    /// cannot make a normally played note fail to open the gate.
    pub maximum_open_dbfs: f64,
    /// The open threshold sits this far above the noise floor, in dB.
    pub margin_db: f64,
    /// The gate closes only this far below the open threshold, in dB, so a level hovering at the
    /// threshold does not chatter.
    pub hysteresis_db: f64,
    /// Once below the close threshold, the gate stays open this long, in seconds, so the gaps
    /// between notes, or the dip of a beating pair of strings, do not close it.
    pub hold_seconds: f64,
}

impl Default for GateConfig {
    /// 10 ms blocks, a 2 s floor, an open threshold of `clamp(floor + 10 dB, −50, −20)` dBFS,
    /// 6 dB of hysteresis and a 250 ms hold.
    fn default() -> Self {
        Self {
            block_seconds: 0.010,
            floor_seconds: 2.0,
            minimum_open_dbfs: -50.0,
            maximum_open_dbfs: -20.0,
            margin_db: 10.0,
            hysteresis_db: 6.0,
            hold_seconds: 0.250,
        }
    }
}

/// Why a [`GateConfig`] was rejected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GateError {
    /// A block is shorter than one sample at the given rate, or not finite.
    Block(f64),
    /// The floor history does not hold at least one block, or is longer than a minute.
    Floor(f64),
    /// The thresholds are not ordered `minimum ≤ maximum`, or a margin, hysteresis or hold is
    /// negative or not finite.
    Thresholds,
    /// The sample rate is zero.
    SampleRate(u32),
}

impl fmt::Display for GateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block(seconds) => write!(f, "gate block of {seconds} s is out of range"),
            Self::Floor(seconds) => write!(f, "gate floor history of {seconds} s is out of range"),
            Self::Thresholds => write!(f, "gate thresholds, hysteresis or hold are inconsistent"),
            Self::SampleRate(rate) => write!(f, "sample rate {rate} Hz is not positive"),
        }
    }
}

impl std::error::Error for GateError {}

/// The level reported for a block of digital silence, in dBFS, instead of `−∞`.
pub const SILENCE_DBFS: f64 = -200.0;

/// The gate's state after a block ([`NoiseGate::state`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateState {
    /// Nothing is being played.
    Closed,
    /// Open only by its hold: the level has fallen below the close threshold, in a gap between
    /// notes or as a note dies away. Taken to mean nothing new is played: a note started under the
    /// close threshold during the hold is not told apart from the gap.
    Holding,
    /// Something is being played: the level is at or above the close threshold.
    Open,
}

impl GateState {
    /// Whether the gate is open, by its level or by its hold.
    #[must_use]
    pub fn is_open(self) -> bool {
        self != Self::Closed
    }
}

/// An RMS gate with a threshold that adapts to the room.
///
/// Every block (an [`RmsMeter`] window, 10 ms by default) it compares the block's level `L` with
///
/// ```text
///   open  = clamp(max(minimum, floor + margin), minimum, maximum)
///   close = open − hysteresis
/// ```
///
/// A closed gate opens when `L ≥ open`. An open gate restarts its hold whenever `L ≥ close`, and
/// closes once the hold has run out. The floor is the minimum block level over the last
/// `floor_seconds` *of learned blocks*. **It learns while the gate is closed, and while it is open
/// only when told the open gate is unpitched** (below). A floor that learned from every open block
/// would, two seconds into a held note, take the note for the noise, raise the threshold above it
/// and close the gate on the player. Minimum statistics (Martin, *IEEE Trans. Speech Audio
/// Process.* 9(5), 2001) is what keeps one loud block from moving it.
///
/// The decisions depend only on the sample sequence, never on how it is cut into blocks. Every
/// buffer is allocated by [`NoiseGate::new`]; processing is allocation-free and panic-free.
///
/// A room louder than the minimum threshold opens the gate, and on its own the gate cannot tell
/// that room from a note. The analysis side can: when it has found no clear pitch
/// for [`crate::chain::UNPITCHED_SECONDS`] it says so ([`NoiseGate::set_unpitched`]), and the
/// floor then learns while the gate is open, so the threshold rises over the room and the gate
/// closes (`docs/PRODUCT_SPEC.md` §2: ambient noise raises the gate). A held note keeps a clear
/// pitch, so it is never learned.
///
/// ```
/// use diapason_dsp::gate::{GateConfig, NoiseGate};
///
/// let mut gate = NoiseGate::new(48_000, GateConfig::default())?;
/// gate.process(&[0.0; 4_800]);
/// assert!(!gate.is_open());
/// gate.process(&[0.1; 480]); // −20 dBFS
/// assert!(gate.is_open());
/// # Ok::<(), diapason_dsp::gate::GateError>(())
/// ```
#[derive(Debug, Clone)]
pub struct NoiseGate {
    config: GateConfig,
    meter: RmsMeter,
    block: usize,
    in_block: usize,
    /// Learned block levels in dBFS (closed, or open and unpitched), a ring of `history.len()`
    /// entries.
    history: Vec<f64>,
    learned: usize,
    next: usize,
    floor_dbfs: f64,
    level_dbfs: f64,
    open: bool,
    hold_blocks: usize,
    hold_left: usize,
    unpitched: bool,
}

impl NoiseGate {
    /// A closed gate for `sample_rate`, with an empty floor history.
    ///
    /// # Errors
    ///
    /// [`GateError`] if a parameter is out of range.
    pub fn new(sample_rate: u32, config: GateConfig) -> Result<Self, GateError> {
        if sample_rate == 0 {
            return Err(GateError::SampleRate(sample_rate));
        }
        let rate = f64::from(sample_rate);
        let block = index_from_f64((config.block_seconds * rate).round());
        if !(config.block_seconds.is_finite() && block >= 1) {
            return Err(GateError::Block(config.block_seconds));
        }
        let blocks = index_from_f64((config.floor_seconds / config.block_seconds).round());
        if !(config.floor_seconds.is_finite() && blocks >= 1 && config.floor_seconds <= 60.0) {
            return Err(GateError::Floor(config.floor_seconds));
        }
        let ordered = config.minimum_open_dbfs <= config.maximum_open_dbfs;
        let finite_non_negative = [config.margin_db, config.hysteresis_db, config.hold_seconds]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0);
        if !(ordered && finite_non_negative) {
            return Err(GateError::Thresholds);
        }
        let window = u32::try_from(block).map_err(|_| GateError::Block(config.block_seconds))?;
        Ok(Self {
            config,
            meter: RmsMeter::new(window),
            block,
            in_block: 0,
            history: vec![SILENCE_DBFS; blocks],
            learned: 0,
            next: 0,
            floor_dbfs: SILENCE_DBFS,
            level_dbfs: SILENCE_DBFS,
            open: false,
            hold_blocks: index_from_f64((config.hold_seconds / config.block_seconds).round()),
            hold_left: 0,
            unpitched: false,
        })
    }

    /// Whether something is being played.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether the gate is closed, open by its level, or open only by its hold.
    #[must_use]
    pub fn state(&self) -> GateState {
        if !self.open {
            GateState::Closed
        } else if self.hold_left < self.hold_blocks {
            GateState::Holding
        } else {
            GateState::Open
        }
    }

    /// Samples per decision.
    #[must_use]
    pub fn block_len(&self) -> usize {
        self.block
    }

    /// The learned noise floor, in dBFS; [`SILENCE_DBFS`] before anything has been learned.
    #[must_use]
    pub fn floor_dbfs(&self) -> f32 {
        to_f32(self.floor_dbfs)
    }

    /// The level of the last complete block, in dBFS.
    #[must_use]
    pub fn level_dbfs(&self) -> f32 {
        to_f32(self.level_dbfs)
    }

    /// The level a block must reach to open the gate, in dBFS.
    #[must_use]
    pub fn open_threshold_dbfs(&self) -> f32 {
        to_f32(self.open_threshold())
    }

    /// Whether the analysis has found no clear pitch for a while with the gate open: if so, the
    /// open gate is taken to be noise, and the floor learns from it too.
    pub fn set_unpitched(&mut self, unpitched: bool) {
        self.unpitched = unpitched;
    }

    fn open_threshold(&self) -> f64 {
        let c = &self.config;
        (self.floor_dbfs + c.margin_db)
            .max(c.minimum_open_dbfs)
            .min(c.maximum_open_dbfs)
    }

    /// Take every sample of `samples`, in order, deciding at each block boundary.
    pub fn process(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.meter.process_sample(sample);
            self.in_block += 1;
            if self.in_block == self.block {
                self.in_block = 0;
                self.decide(f64::from(self.meter.last()));
            }
        }
    }

    fn decide(&mut self, rms: f64) {
        self.level_dbfs = if rms > 0.0 {
            (20.0 * rms.log10()).max(SILENCE_DBFS)
        } else {
            SILENCE_DBFS
        };
        if self.open && self.unpitched {
            self.learn();
        }
        let open = self.open_threshold();
        if self.open {
            if self.level_dbfs >= open - self.config.hysteresis_db {
                self.hold_left = self.hold_blocks;
            } else if self.hold_left > 0 {
                self.hold_left -= 1;
            } else {
                self.open = false;
            }
        } else if self.level_dbfs >= open {
            self.open = true;
            self.hold_left = self.hold_blocks;
        } else {
            self.learn();
        }
    }

    /// Add the current block to the learned history and take the floor as its minimum.
    fn learn(&mut self) {
        if let Some(slot) = self.history.get_mut(self.next) {
            *slot = self.level_dbfs;
        }
        self.next = (self.next + 1) % self.history.len().max(1);
        self.learned = (self.learned + 1).min(self.history.len());
        self.floor_dbfs = self
            .history
            .iter()
            .take(self.learned)
            .copied()
            .fold(f64::INFINITY, f64::min);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{Noise, sine};
    use proptest::prelude::*;

    const RATE: u32 = 48_000;

    fn gate() -> NoiseGate {
        NoiseGate::new(RATE, GateConfig::default()).unwrap()
    }

    /// `seconds` of a 440 Hz sine whose RMS is `dbfs`.
    fn tone(dbfs: f64, seconds: f64) -> Vec<f32> {
        let mut out = vec![0.0_f32; index_from_f64(seconds * f64::from(RATE))];
        let amplitude = 10_f64.powf(dbfs / 20.0) * std::f64::consts::SQRT_2;
        sine(&mut out, 440.0, RATE, amplitude, 0.0);
        out
    }

    /// `seconds` of uniform noise whose RMS is `dbfs` (peak `√3` times the RMS).
    fn noise(dbfs: f64, seconds: f64, seed: u64) -> Vec<f32> {
        let mut out = vec![0.0_f32; index_from_f64(seconds * f64::from(RATE))];
        Noise::new(seed).fill(&mut out, 10_f64.powf(dbfs / 20.0) * 3_f64.sqrt());
        out
    }

    /// Feed `signal` one block at a time; the gate state after each block.
    fn states(gate: &mut NoiseGate, signal: &[f32]) -> Vec<bool> {
        signal
            .chunks(gate.block_len())
            .map(|block| {
                gate.process(block);
                gate.is_open()
            })
            .collect()
    }

    #[test]
    fn silence_stays_closed() {
        let mut gate = gate();
        assert!(
            states(&mut gate, &vec![0.0; 5 * 48_000])
                .iter()
                .all(|&s| !s)
        );
        assert_eq!(gate.floor_dbfs().to_bits(), (-200.0_f32).to_bits());
        assert_eq!(gate.open_threshold_dbfs().to_bits(), (-50.0_f32).to_bits());
    }

    #[test]
    fn a_minus_40_dbfs_tone_opens_it_within_one_block() {
        let mut gate = gate();
        gate.process(&noise(-70.0, 1.0, 1));
        assert!(!gate.is_open());
        let opened = states(&mut gate, &tone(-40.0, 0.1));
        assert_eq!(opened.iter().position(|&s| s), Some(0));
        // 4.4 periods per block, so the block RMS is within a fraction of a dB of the tone's.
        assert!(
            (gate.level_dbfs() + 40.0).abs() < 0.3,
            "{}",
            gate.level_dbfs()
        );
    }

    /// The test that fails if the floor learns from an open gate that is not unpitched: two
    /// seconds in, the held note would be the floor, the threshold 10 dB above it, and the gate
    /// would close.
    #[test]
    fn a_note_held_for_ten_seconds_stays_open() {
        let mut gate = gate();
        gate.process(&noise(-65.0, 2.0, 2));
        let held = states(&mut gate, &tone(-30.0, 10.0));
        assert!(
            held.iter().all(|&s| s),
            "closed after {:?} blocks",
            held.iter().position(|&s| !s)
        );
        assert!(gate.floor_dbfs() < -64.0, "{}", gate.floor_dbfs());
    }

    #[test]
    fn room_noise_raises_the_threshold() {
        let mut gate = gate();
        gate.process(&noise(-56.0, 3.0, 3));
        assert!(!gate.is_open());
        // The floor is the quietest 10 ms block, a little under the noise's RMS.
        let floor = gate.floor_dbfs();
        assert!((-58.0..-56.0).contains(&floor), "{floor}");
        let threshold = gate.open_threshold_dbfs();
        assert!(
            (threshold - (floor + 10.0)).abs() < 1e-4 && threshold > -50.0,
            "{threshold}"
        );
        // A tone just over the default −50 dBFS no longer opens it; a louder one does.
        let mut quiet = noise(-56.0, 0.1, 4);
        let loud = tone(-49.0, 0.1);
        for (q, t) in quiet.iter_mut().zip(&loud) {
            *q += t;
        }
        assert!(states(&mut gate, &quiet).iter().all(|&s| !s));
        assert!(states(&mut gate, &tone(-40.0, 0.1)).iter().any(|&s| s));
    }

    /// Opened at −40 dBFS, a second at −53 dBFS sits between the open threshold (−50) and the close
    /// threshold (−56): open, thanks to the hysteresis, and closed without it.
    #[test]
    fn the_hysteresis_holds_a_level_between_the_thresholds() {
        for (hysteresis_db, stays_open) in [(6.0, true), (0.0, false)] {
            let config = GateConfig {
                hysteresis_db,
                ..GateConfig::default()
            };
            let mut gate = NoiseGate::new(RATE, config).unwrap();
            gate.process(&tone(-40.0, 0.2));
            assert!(gate.is_open());
            let states = states(&mut gate, &tone(-53.0, 1.0));
            assert_eq!(
                states.iter().all(|&s| s),
                stays_open,
                "hysteresis {hysteresis_db} dB"
            );
        }
    }

    /// Start-up noise above the minimum threshold opens the gate. Told the open gate has no pitch,
    /// the floor learns the noise, the threshold rises over it, and the gate closes: in one block
    /// of learning and the 250 ms hold.
    #[test]
    fn an_unpitched_open_gate_learns_the_room_and_closes() {
        let mut gate = gate();
        gate.process(&noise(-45.0, 2.0, 7));
        assert!(gate.is_open(), "−45 dBFS is over the −50 dBFS minimum");
        gate.set_unpitched(true);
        let closed = states(&mut gate, &noise(-45.0, 1.0, 8))
            .iter()
            .position(|&s| !s);
        assert_eq!(closed, Some(25));
        assert!(
            gate.open_threshold_dbfs() > -40.0,
            "{}",
            gate.open_threshold_dbfs()
        );
        // Closed, it stays closed in the same room, and a note still opens it.
        gate.set_unpitched(false);
        assert!(states(&mut gate, &noise(-45.0, 1.0, 9)).iter().all(|&s| !s));
        assert!(states(&mut gate, &tone(-20.0, 0.05)).iter().any(|&s| s));
    }

    #[test]
    fn the_threshold_is_clamped() {
        let mut gate = NoiseGate::new(
            RATE,
            GateConfig {
                maximum_open_dbfs: -45.0,
                ..GateConfig::default()
            },
        )
        .unwrap();
        gate.process(&noise(-54.0, 3.0, 5));
        assert_eq!(gate.open_threshold_dbfs().to_bits(), (-45.0_f32).to_bits());
    }

    #[test]
    fn a_tone_within_the_hold_time_keeps_it_open_and_after_it_reopens_at_once() {
        let mut gate = gate();
        gate.process(&tone(-30.0, 0.5));
        assert!(gate.is_open());
        // A 200 ms gap, under the 250 ms hold: never closes.
        assert!(states(&mut gate, &vec![0.0; 9_600]).iter().all(|&s| s));
        assert!(states(&mut gate, &tone(-30.0, 0.2)).iter().all(|&s| s));
        // A 300 ms gap closes it after the hold: 25 blocks of hold, then the first block below.
        let gap = states(&mut gate, &vec![0.0; 14_400]);
        assert_eq!(gap.iter().position(|&s| !s), Some(25));
        // And the next note opens it on its first block.
        assert!(states(&mut gate, &tone(-40.0, 0.05))[0]);
    }

    /// The state says why the gate is open: by the level, or only by the hold over a gap.
    #[test]
    fn the_state_tells_a_held_gap_from_a_played_note() {
        let mut gate = gate();
        assert_eq!(gate.state(), GateState::Closed);
        gate.process(&tone(-30.0, 0.5));
        assert_eq!(gate.state(), GateState::Open);
        let block = vec![0.0; gate.block_len()];
        let mut gap = Vec::new();
        for _ in 0..30 {
            gate.process(&block);
            gap.extend([gate.state()]);
        }
        // 25 blocks of hold, then closed; open the whole time by `is_open` until then.
        assert!(gap[..25].iter().all(|&s| s == GateState::Holding));
        assert!(gap[25..].iter().all(|&s| s == GateState::Closed));
        gate.process(&tone(-30.0, 0.05));
        assert_eq!(gate.state(), GateState::Open);
    }

    #[test]
    fn invalid_configurations_are_rejected() {
        let ok = GateConfig::default();
        assert_eq!(NoiseGate::new(0, ok).unwrap_err(), GateError::SampleRate(0));
        let bad = |config| NoiseGate::new(RATE, config).unwrap_err();
        assert!(matches!(
            bad(GateConfig {
                block_seconds: 0.0,
                ..ok
            }),
            GateError::Block(_)
        ));
        assert!(matches!(
            bad(GateConfig {
                floor_seconds: 0.001,
                ..ok
            }),
            GateError::Floor(_)
        ));
        assert_eq!(
            bad(GateConfig {
                minimum_open_dbfs: -10.0,
                ..ok
            }),
            GateError::Thresholds
        );
        assert_eq!(
            bad(GateConfig {
                hold_seconds: f64::NAN,
                ..ok
            }),
            GateError::Thresholds
        );
    }

    proptest! {
        /// Decisions depend on the signal only, never on how it was cut into blocks.
        #[test]
        fn decisions_are_independent_of_block_boundaries(
            blocks in prop::collection::vec(1_usize..2_000, 1..60),
        ) {
            let total: usize = blocks.iter().sum();
            let mut signal = noise(-60.0, 1.0, 6);
            signal.extend(tone(-35.0, 0.3));
            signal.extend(vec![0.0; 24_000]);
            signal.truncate(total.min(signal.len()));
            let mut whole = gate();
            let mut split = gate();
            whole.process(&signal);
            let mut start = 0;
            for size in blocks {
                let end = (start + size).min(signal.len());
                split.process(&signal[start..end]);
                start = end;
            }
            assert_eq!(whole.is_open(), split.is_open());
            assert_eq!(whole.floor_dbfs().to_bits(), split.floor_dbfs().to_bits());
            assert_eq!(whole.level_dbfs().to_bits(), split.level_dbfs().to_bits());
        }
    }
}

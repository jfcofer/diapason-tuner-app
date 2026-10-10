//! How much queued microphone input to discard: the arithmetic behind the Android backend's
//! backlog shedding, kept free of platform calls so it is tested on any host (`docs/adr/0025`).

/// Callbacks per window of the floor: about 1.3 s of 20 ms callbacks. The floor is the lowest gap
/// seen over the last one to two windows.
const FLOOR_WINDOW: u32 = 64;

/// The lowest gap between an input's frames written and read seen over the last one to two
/// windows of callbacks. Two running minimums, rotated every [`FLOOR_WINDOW`] callbacks, so it costs
/// two comparisons and no memory: real-time safe.
///
/// Measuring against a floor rather than zero is what the Redmi taught. On the legacy capture path
/// the counters lead what a read can return by a pipeline offset, so `written - read` is never near
/// zero, and taken as backlog it shed ~20 % of the microphone.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Floor {
    previous: i64,
    current: i64,
    seen: u32,
}

impl Floor {
    pub(crate) const fn new() -> Self {
        Self {
            previous: i64::MAX,
            current: i64::MAX,
            seen: 0,
        }
    }

    /// Record one callback's gap and return the floor, which includes it.
    pub(crate) fn observe(&mut self, gap: i64) -> i64 {
        self.current = self.current.min(gap);
        self.seen += 1;
        let floor = self.previous.min(self.current);
        if self.seen >= FLOOR_WINDOW {
            self.previous = self.current;
            self.current = i64::MAX;
            self.seen = 0;
        }
        floor
    }
}

/// Frames to discard when the gap stands `gap - floor` above its floor: nothing until that exceeds
/// a burst (how unevenly the microphone delivers) plus the slack, then everything above the slack.
pub(crate) fn frames_to_shed(gap: i64, floor: i64, burst: usize, slack: usize) -> usize {
    let backlog = usize::try_from(gap.saturating_sub(floor)).unwrap_or(0);
    if backlog <= slack.saturating_add(burst) {
        0
    } else {
        backlog - slack
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BURST: usize = 960;
    const SLACK: usize = 240;

    /// Feed `gaps` through a floor and add up what would be shed.
    fn shed_over(floor: &mut Floor, gaps: impl IntoIterator<Item = i64>) -> usize {
        gaps.into_iter()
            .map(|gap| {
                let base = floor.observe(gap);
                frames_to_shed(gap, base, BURST, SLACK)
            })
            .sum()
    }

    #[test]
    fn a_constant_offset_with_delivery_jitter_is_never_backlog() {
        // What the Redmi showed: ~1700 frames the counters report but no read returns, plus one
        // burst of jitter from delivery granularity.
        let mut floor = Floor::new();
        let steady = (0..1000).map(|i| 1700 + (i % 5) * 200);
        assert_eq!(shed_over(&mut floor, steady), 0);
    }

    #[test]
    fn a_stall_sized_rise_is_shed_down_to_the_slack() {
        let mut floor = Floor::new();
        shed_over(&mut floor, std::iter::repeat_n(1700, 100));
        // A 200 ms stall at 48 kHz: ~9600 frames above the floor.
        let base = floor.observe(1700 + 9600);
        assert_eq!(
            frames_to_shed(1700 + 9600, base, BURST, SLACK),
            9600 - SLACK
        );
    }

    #[test]
    fn a_rise_under_a_burst_and_the_slack_is_left_alone() {
        let mut floor = Floor::new();
        shed_over(&mut floor, std::iter::repeat_n(1700, 100));
        let base = floor.observe(1700 + 1200);
        assert_eq!(frames_to_shed(1700 + 1200, base, BURST, SLACK), 0);
    }

    #[test]
    fn a_lasting_change_of_offset_is_adopted_within_two_windows() {
        // A route change can move the pipeline offset for good. Within two windows the floor
        // follows, so the new offset stops reading as backlog.
        let mut floor = Floor::new();
        shed_over(&mut floor, std::iter::repeat_n(1700, 100));
        let higher = 1700 + 3000;
        for _ in 0..(2 * FLOOR_WINDOW) {
            floor.observe(higher);
        }
        let base = floor.observe(higher);
        assert_eq!(frames_to_shed(higher, base, BURST, SLACK), 0);
    }
}

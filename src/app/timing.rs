//! Turning real time into fixed update steps

use std::time::Duration;

/// The most update steps one frame runs to catch up. Time past that is dropped
const MAX_CATCH_UP_STEPS: u32 = 5;

/// Turns real time into a whole number of fixed-length update steps
pub(super) struct FixedTimestep {
    dt: Duration,
    lag: Duration,
}

impl FixedTimestep {
    pub(super) fn new(dt: Duration) -> Self {
        Self {
            dt,
            lag: Duration::ZERO,
        }
    }

    /// How many steps to run for `elapsed` more real time
    pub(super) fn advance(&mut self, elapsed: Duration) -> u32 {
        self.lag += elapsed;
        let max_lag = self.dt * MAX_CATCH_UP_STEPS;
        if self.lag > max_lag {
            log::debug!("running behind, skipping {:?}", self.lag - max_lag);
            self.lag = max_lag;
        }
        let steps = (self.lag.as_nanos() / self.dt.as_nanos()) as u32;
        self.lag -= self.dt * steps;
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: Duration = Duration::from_micros(16_667);

    #[test]
    fn one_step_per_dt_and_leftovers_carry_over() {
        let mut timestep = FixedTimestep::new(DT);
        assert_eq!(timestep.advance(DT), 1);
        assert_eq!(timestep.advance(DT / 2), 0);
        assert_eq!(timestep.advance(DT / 2), 1);
        assert_eq!(timestep.advance(DT * 5 / 2), 2);
        assert_eq!(timestep.advance(DT / 2), 1);
    }

    #[test]
    fn a_long_frame_only_catches_up_so_far() {
        let mut timestep = FixedTimestep::new(DT);
        assert_eq!(
            timestep.advance(Duration::from_secs(10)),
            MAX_CATCH_UP_STEPS
        );
        // The rest of the 10 seconds is gone, so things go back to normal right away
        assert_eq!(timestep.advance(DT), 1);
    }

    #[test]
    fn slow_updates_dont_snowball() {
        // Every step takes twice as long as the time it covers
        let mut timestep = FixedTimestep::new(DT);
        let mut steps = 1;
        for _ in 0..20 {
            steps = timestep.advance(DT * 2 * steps);
            assert!(steps <= MAX_CATCH_UP_STEPS, "{steps} steps in one frame");
        }
    }
}

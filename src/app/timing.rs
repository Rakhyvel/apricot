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

/// Counts frames drawn to work out the frame rate, over windows of about 5 seconds
pub(super) struct FpsCounter {
    window_start: Duration,
    frames: u32,
}

impl FpsCounter {
    /// Start counting at `now`, the time since the app started
    pub(super) fn new(now: Duration) -> Self {
        Self {
            window_start: now,
            frames: 0,
        }
    }

    pub(super) fn frame_drawn(&mut self) {
        self.frames += 1;
    }

    /// The frame rate so far at `now`, once there's been at least a second to measure
    pub(super) fn update(&mut self, now: Duration) -> Option<u32> {
        let elapsed = now - self.window_start;
        let rate = (elapsed >= Duration::from_secs(1))
            .then(|| (self.frames as f64 / elapsed.as_secs_f64()).round() as u32);
        if elapsed > Duration::from_secs(5) {
            log::debug!("{} fps over the last 5 seconds", self.frames / 5);
            self.window_start = now;
            self.frames = 0;
        }
        rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draw `frames` frames, then check the rate at `now`
    fn after(fps: &mut FpsCounter, frames: u32, now: Duration) -> Option<u32> {
        for _ in 0..frames {
            fps.frame_drawn();
        }
        fps.update(now)
    }

    #[test]
    fn fps_waits_for_a_second_then_counts_frames_per_second() {
        let mut fps = FpsCounter::new(Duration::from_secs(2));
        assert_eq!(after(&mut fps, 30, Duration::from_millis(2500)), None);
        assert_eq!(after(&mut fps, 30, Duration::from_secs(3)), Some(60));
        assert_eq!(after(&mut fps, 60, Duration::from_secs(4)), Some(60));
        assert_eq!(after(&mut fps, 30, Duration::from_secs(5)), Some(50));
    }

    #[test]
    fn fps_starts_a_fresh_window_after_five_seconds() {
        let mut fps = FpsCounter::new(Duration::ZERO);
        assert_eq!(after(&mut fps, 300, Duration::from_secs(5)), Some(60));
        // Past 5 seconds it reports the whole window one last time, then starts a new one
        assert_eq!(after(&mut fps, 1, Duration::from_millis(5100)), Some(59));
        // So the next report needs another second
        assert_eq!(after(&mut fps, 10, Duration::from_millis(5600)), None);
        assert_eq!(after(&mut fps, 50, Duration::from_millis(6100)), Some(60));
    }

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

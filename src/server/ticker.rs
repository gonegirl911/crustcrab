use spin_sleep::SpinSleeper;
use std::time::{Duration, Instant};

pub struct Ticker {
    sleeper: SpinSleeper,
    duration: Duration,
    prev: Instant,
}

impl Ticker {
    const NATIVE_ACCURACY: Duration = Duration::from_millis(5);

    pub fn start(ticks_per_second: u64) -> Self {
        Self {
            sleeper: SpinSleeper::new(Self::NATIVE_ACCURACY.as_nanos() as u32),
            duration: Duration::from_millis(1000 / ticks_per_second),
            prev: Instant::now(),
        }
    }

    pub fn wait<T, F: FnOnce() -> T>(&mut self, f: F) -> T {
        self.sleeper.sleep(self.rem());
        self.prev = Instant::now();
        f()
    }

    fn rem(&self) -> Duration {
        self.duration.saturating_sub(self.prev.elapsed())
    }
}

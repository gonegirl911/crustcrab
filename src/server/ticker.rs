use spin_sleep::SpinSleeper;
use std::time::{Duration, Instant};

pub struct Ticker {
    sleeper: SpinSleeper,
    dt: Duration,
    prev: Instant,
}

impl Ticker {
    const NATIVE_ACCURACY: Duration = Duration::from_millis(5);

    pub fn start(ticks_per_second: u32) -> Self {
        Self {
            sleeper: SpinSleeper::new(Self::NATIVE_ACCURACY.as_nanos() as u32),
            dt: Duration::from_secs(1) / ticks_per_second,
            prev: Instant::now(),
        }
    }

    pub fn wait<T, F: FnOnce() -> T>(&mut self, f: F) -> T {
        self.sleeper.sleep(self.rem());
        self.prev = Instant::now();
        f()
    }

    fn rem(&self) -> Duration {
        self.dt.saturating_sub(self.prev.elapsed())
    }
}

use std::time::{Duration, Instant};

pub struct Ticker {
    prev: Instant,
}

impl Ticker {
    const TICKS_PER_SECOND: u64 = 20;
    const TICK_DURATION: Duration = Duration::from_millis(1000 / Self::TICKS_PER_SECOND);

    pub fn start() -> Self {
        Self {
            prev: Instant::now(),
        }
    }

    pub fn wait<T, F: FnOnce() -> T>(&mut self, on_finish: F) -> T {
        spin_sleep::sleep(Self::TICK_DURATION.saturating_sub(self.prev.elapsed()));
        self.prev = Instant::now();
        on_finish()
    }
}

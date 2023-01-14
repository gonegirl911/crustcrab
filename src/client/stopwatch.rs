use std::time::{Duration, Instant};

pub struct Stopwatch {
    prev: Instant,
}

impl Stopwatch {
    pub fn start() -> Self {
        Self {
            prev: Instant::now(),
        }
    }

    pub fn lap(&mut self) -> Duration {
        let now = Instant::now();
        let dt = now.duration_since(self.prev);
        self.prev = now;
        dt
    }
}

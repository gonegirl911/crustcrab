use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Budget<T> {
    limit: T,
    consumed: T,
}

impl<T: Default> Budget<T> {
    pub fn reset(&mut self, limit: T) {
        self.limit = limit;
        self.consumed = Default::default();
    }
}

impl Budget<Duration> {
    pub fn measure(&mut self) -> TimedScope<'_> {
        TimedScope {
            budget: self,
            started_at: Instant::now(),
        }
    }

    fn consume(&mut self, amount: Duration) {
        self.consumed += amount;
    }

    fn remaining(&self) -> Duration {
        self.limit.saturating_sub(self.consumed)
    }
}

pub struct TimedScope<'a> {
    budget: &'a mut Budget<Duration>,
    started_at: Instant,
}

impl TimedScope<'_> {
    pub fn remaining(&self) -> Duration {
        self.budget.remaining()
    }
}

impl Drop for TimedScope<'_> {
    fn drop(&mut self) {
        self.budget.consume(self.started_at.elapsed());
    }
}

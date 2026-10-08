use std::time::{Duration, Instant};

pub struct Pacer {
    gap: Duration,
    stamped_at: Option<Instant>,
}

impl Pacer {
    pub fn new(gap: Duration) -> Self {
        Self {
            gap,
            stamped_at: None,
        }
    }

    pub fn fire<T, F: FnOnce() -> T>(&mut self, now: Instant, f: F) -> Option<T> {
        self.admit(now).then(f)
    }

    pub fn admit(&mut self, now: Instant) -> bool {
        if self.is_due(now) {
            self.stamp(now);
            true
        } else {
            false
        }
    }

    fn is_due(&self, now: Instant) -> bool {
        self.stamped_at
            .is_none_or(|stamped_at| now.duration_since(stamped_at) >= self.gap)
    }

    fn stamp(&mut self, now: Instant) {
        self.stamped_at = Some(now);
    }
}

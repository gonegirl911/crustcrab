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

pub struct Paced<T> {
    pub mut(self) value: T,
    pacer: Pacer,
}

impl<T> Paced<T> {
    pub fn new(value: T, gap: Duration) -> Self {
        Self {
            value,
            pacer: Pacer::new(gap),
        }
    }

    pub fn admit<F: FnOnce(&mut T)>(&mut self, now: Instant, action: F) -> &T {
        if self.pacer.is_due(now) {
            action(&mut self.value);
        }
        &self.value
    }
}

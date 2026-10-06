use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Pacer {
    stamped_at: Option<Instant>,
}

impl Pacer {
    pub fn is_due(&self, gap: Duration, now: Instant) -> bool {
        self.stamped_at
            .is_none_or(|stamped_at| now.duration_since(stamped_at) >= gap)
    }

    pub fn stamp(&mut self, now: Instant) {
        self.stamped_at = Some(now);
    }

    pub fn clear(&mut self) {
        self.stamped_at = None;
    }
}

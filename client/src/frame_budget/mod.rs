mod cpu;

use base::flow::pacer::Paced;
use std::time::{Duration, Instant};

pub(super) struct FrameBudget {
    target_interval: Paced<Duration>,
    pub(super) cpu: cpu::FrameBudget,
}

impl FrameBudget {
    const DEFAULT_TARGET_INTERVAL: Duration = Duration::from_nanos(1000000000 / 60);
    const TARGET_INTERVAL_REFRESH_GAP: Duration = Duration::from_secs(1);

    pub(super) fn begin_frame(
        &mut self,
        now: Instant,
        target_interval: impl FnOnce() -> Option<Duration>,
    ) {
        let target_interval = self.target_interval.admit(now, |value| {
            if let Some(interval) = target_interval() {
                *value = interval;
            }
        });
        self.cpu.begin_frame(*target_interval);
    }

    pub(super) fn finish_frame(&mut self) {
        self.cpu.finish_frame();
    }

    pub(super) fn discard_frame(&mut self) {
        self.cpu.discard_frame();
    }
}

impl Default for FrameBudget {
    fn default() -> Self {
        Self {
            target_interval: Paced::new(
                Self::DEFAULT_TARGET_INTERVAL,
                Self::TARGET_INTERVAL_REFRESH_GAP,
            ),
            cpu: Default::default(),
        }
    }
}

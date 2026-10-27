use base::{flow::budget::Budget, math::smoothing};
use std::time::Duration;

#[derive(Default)]
pub(crate) struct FrameBudget {
    pub(crate) time: TimeBudget,
}

impl FrameBudget {
    pub(super) fn begin_frame(&mut self, target_interval: Duration) {
        self.time.begin_sample(target_interval);
    }

    pub(super) fn finish_frame(&mut self) {
        self.time.finish_sample();
    }

    pub(super) fn discard_frame(&mut self) {
        self.time.discard_sample();
    }
}

#[derive(Default)]
pub(crate) struct TimeBudget {
    pub(crate) baseline: smoothing::Estimator<Duration>,
    pub(crate) slack: Budget<Duration>,
}

impl TimeBudget {
    const BASELINE_RISE_WEIGHT: f32 = 0.5;
    const BASELINE_FALL_WEIGHT: f32 = 0.05;
    const HEADROOM: Duration = Duration::from_millis(1);

    fn begin_sample(&mut self, target_interval: Duration) {
        let limit = self.baseline.estimate().map_or_default(|baseline| {
            target_interval
                .saturating_sub(baseline)
                .saturating_sub(Self::HEADROOM)
        });
        self.baseline.begin_sample();
        self.slack.reset(limit);
    }

    fn finish_sample(&mut self) {
        self.baseline.finish_sample(smoothing::Weight::asymmetric(
            Self::BASELINE_RISE_WEIGHT,
            Self::BASELINE_FALL_WEIGHT,
        ));
    }

    fn discard_sample(&mut self) {
        self.baseline.discard_sample();
    }
}

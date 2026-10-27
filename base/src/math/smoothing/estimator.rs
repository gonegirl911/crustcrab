use super::{ExponentialSmoother, Weight};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Estimator<T> {
    smoother: ExponentialSmoother,
    sample: T,
    is_pending: bool,
}

impl<T: Default> Estimator<T> {
    pub fn begin_sample(&mut self) {
        self.sample = Default::default();
        self.is_pending = true;
    }

    pub fn discard_sample(&mut self) {
        self.sample = Default::default();
        self.is_pending = false;
    }
}

impl Estimator<Duration> {
    pub fn measure(&mut self) -> TimedScope<'_> {
        TimedScope {
            estimator: self,
            started_at: Instant::now(),
        }
    }

    pub fn finish_sample(&mut self, weight: Weight) {
        if self.is_pending {
            self.smoother.smooth(self.sample.as_secs_f32(), weight);
        }
        self.discard_sample();
    }

    pub fn estimate(&self) -> Option<Duration> {
        self.smoother.value.map(Duration::from_secs_f32)
    }
}

pub struct TimedScope<'a> {
    estimator: &'a mut Estimator<Duration>,
    started_at: Instant,
}

impl Drop for TimedScope<'_> {
    fn drop(&mut self) {
        self.estimator.sample += self.started_at.elapsed();
    }
}

use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub struct Policer {
    span: Duration,
    max_arrivals: usize,
    arrivals: VecDeque<Instant>,
}

impl Policer {
    pub fn new(span: Duration, max_arrivals: usize) -> Self {
        Self {
            span,
            max_arrivals,
            arrivals: Default::default(),
        }
    }

    pub fn police(&mut self, now: Instant) -> bool {
        self.arrivals.push_back(now);

        while let Some(&arrival) = self.arrivals.front()
            && now.duration_since(arrival) >= self.span
        {
            self.arrivals.pop_front();
        }

        if self.arrivals.len() > self.max_arrivals + 1 {
            self.arrivals.pop_front();
        }

        self.arrivals.len() <= self.max_arrivals
    }
}

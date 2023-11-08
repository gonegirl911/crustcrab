use flume::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

pub struct Ticker {
    dt: Duration,
    next: Instant,
}

impl Ticker {
    pub fn start(ticks_per_second: u32) -> Self {
        let now = Instant::now();
        let dt = Duration::from_secs(1) / ticks_per_second;
        let next = now + dt;
        Self { dt, next }
    }

    pub fn recv_deadline<T>(&mut self, tx: &Receiver<T>) -> Result<T, RecvTimeoutError> {
        match self.deadline().map(|deadline| tx.recv_deadline(deadline)) {
            Some(Err(RecvTimeoutError::Timeout)) | None => {
                self.next += self.dt;
                Err(RecvTimeoutError::Timeout)
            }
            Some(value) => value,
        }
    }

    fn deadline(&self) -> Option<Instant> {
        (Instant::now() < self.next).then_some(self.next)
    }
}

use crossbeam_channel::{Receiver, RecvTimeoutError};
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

    pub fn recv_timeout<T>(&mut self, tx: &Receiver<T>) -> Result<T, RecvTimeoutError> {
        match self.timeout().map(|timeout| tx.recv_timeout(timeout)) {
            Some(Err(RecvTimeoutError::Timeout)) | None => {
                self.next += self.dt;
                Err(RecvTimeoutError::Timeout)
            }
            Some(value) => value,
        }
    }

    fn timeout(&self) -> Option<Duration> {
        self.next.checked_duration_since(Instant::now())
    }
}

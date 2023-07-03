use flume::{Receiver, SendError, Sender};
use once_cell::sync::Lazy;
use std::thread;

pub struct ThreadPool<I, O> {
    in_tx: Sender<I>,
    out_rx: Receiver<O>,
}

impl<I, O> ThreadPool<I, O> {
    pub fn send(&self, input: I) -> Result<(), SendError<I>> {
        self.in_tx.send(input)
    }

    pub fn drain(&self) -> impl Iterator<Item = O> + '_ {
        self.out_rx.drain()
    }
}

impl<I: Send + 'static, O: Send + 'static> ThreadPool<I, O> {
    pub fn new<F: Fn(I) -> O + Copy + Send + 'static>(f: F) -> Self {
        let (in_tx, in_rx) = flume::unbounded();
        let (out_tx, out_rx) = flume::unbounded();

        for _ in 0..*NUM_CPUS {
            let in_rx = in_rx.clone();
            let out_tx = out_tx.clone();
            thread::spawn(move || {
                for input in in_rx {
                    if out_tx.send(f(input)).is_err() {
                        break;
                    }
                }
            });
        }

        Self { in_tx, out_rx }
    }
}

static NUM_CPUS: Lazy<usize> = Lazy::new(num_cpus::get);

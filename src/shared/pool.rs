use crossbeam_channel::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::{
    collections::VecDeque,
    mem::DropGuard,
    num::NonZero,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::Instant,
};

pub fn init(reserved_threads: usize) {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads(reserved_threads))
        .build_global()
        .unwrap();
}

pub struct JobPool<I, O> {
    inner: Arc<Inner<I, O>>,
    out_rx: Receiver<O>,
}

impl<I, O> JobPool<I, O> {
    pub fn try_recv(&self) -> Result<O, TryRecvError> {
        self.out_rx.try_recv()
    }

    pub fn recv_deadline(&self, deadline: Instant) -> Result<O, RecvTimeoutError> {
        self.out_rx.recv_deadline(deadline)
    }
}

impl<I: Send + 'static, O: Send + 'static> JobPool<I, O> {
    pub fn new<J: Fn(I) -> O + Send + Sync + 'static>(reserved_threads: usize, job: J) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads(reserved_threads))
            .build()
            .unwrap()
            .into();
        let (out_tx, out_rx) = crossbeam_channel::unbounded();
        Self {
            inner: Arc::new(Inner {
                pool,
                job: Box::new(job),
                out_tx,
                pending: Default::default(),
            }),
            out_rx,
        }
    }

    pub fn submit(&self, input: I, has_priority: bool) {
        let mut pending = self.inner.pending.lock().unwrap();
        pending.push(input, has_priority);
        Self::dispatch(&self.inner, pending);
    }

    fn dispatch(inner: &Arc<Inner<I, O>>, mut pending: MutexGuard<Pending<I>>) {
        let current_num_threads = rayon::current_num_threads();

        while pending.in_flight < current_num_threads
            && let Some(input) = pending.pop()
        {
            pending.in_flight += 1;

            let inner = inner.clone();
            let pool = inner.pool.clone();
            pool.spawn(move || {
                let inner = DropGuard::new(inner, |inner| {
                    let mut pending = inner.pending.lock().unwrap();
                    pending.in_flight -= 1;
                    Self::dispatch(&inner, pending);
                });

                let output = (inner.job)(input);
                _ = inner.out_tx.send(output);
            });
        }
    }
}

struct Inner<I, O> {
    pool: Arc<rayon::ThreadPool>,
    job: Box<dyn Fn(I) -> O + Send + Sync>,
    out_tx: Sender<O>,
    pending: Mutex<Pending<I>>,
}

struct Pending<I> {
    priority_inputs: VecDeque<I>,
    inputs: VecDeque<I>,
    in_flight: usize,
}

impl<I> Pending<I> {
    fn push(&mut self, input: I, has_priority: bool) {
        if has_priority {
            self.priority_inputs.push_back(input);
        } else {
            self.inputs.push_back(input);
        }
    }

    fn pop(&mut self) -> Option<I> {
        self.priority_inputs
            .pop_front()
            .or_else(|| self.inputs.pop_front())
    }
}

impl<I> Default for Pending<I> {
    fn default() -> Self {
        Self {
            priority_inputs: Default::default(),
            inputs: Default::default(),
            in_flight: 0,
        }
    }
}

fn num_threads(reserved_threads: usize) -> usize {
    thread::available_parallelism()
        .map_or(1, NonZero::get)
        .saturating_sub(reserved_threads)
        .max(1)
}

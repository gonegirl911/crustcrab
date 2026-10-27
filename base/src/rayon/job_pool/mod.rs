mod demotion;
mod queue;
mod scheduler;

use super::available_parallelism;
use crossbeam_channel::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use demotion::demote;
use scheduler::{Admitted, Completed, EnqueueOptions, Scheduler};
use std::{
    hash::Hash,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct JobPool<I, O, K = ()> {
    inner: Arc<Inner<I, O, K>>,
    out_rx: Receiver<Completed<O, K>>,
}

impl<I, O, K> JobPool<I, O, K> {
    fn new<H: Fn(I) -> O + Send + Sync + 'static>(headroom: usize, handler: H) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(available_parallelism(headroom))
            .start_handler(|_| {
                _ = demote();
            })
            .build()
            .unwrap();
        let (out_tx, out_rx) = crossbeam_channel::unbounded();
        Self {
            inner: Arc::new(Inner {
                pool,
                handler: Box::new(handler),
                out_tx,
                scheduler: Default::default(),
                shutdown_requested: false.into(),
            }),
            out_rx,
        }
    }
}

impl<I, O, K: Hash + Eq> JobPool<I, O, K> {
    fn try_recv(&self) -> Result<O, TryRecvError> {
        self.out_rx.try_recv().map(|job| self.finish(job))
    }

    fn recv_deadline(&self, deadline: Instant) -> Result<O, RecvTimeoutError> {
        self.out_rx
            .recv_deadline(deadline)
            .map(|job| self.finish(job))
    }

    fn finish(&self, Completed { output, key }: Completed<O, K>) -> O {
        if let Some(key) = key {
            let mut scheduler = self.inner.scheduler.lock().unwrap();
            let _removed = scheduler.finish(&key);
            drop(scheduler)
        }
        output
    }
}

impl<I, O, K> JobPool<I, O, K>
where
    I: Send + 'static,
    O: Send + 'static,
    K: Hash + Eq + Clone + Send + 'static,
{
    fn submit(&self, input: I, options: SubmitOptions<K>) {
        let SubmitOptions { has_priority, key } = options;

        let mut scheduler = self.inner.scheduler.lock().unwrap();
        let discarded = scheduler.enqueue(
            input,
            EnqueueOptions {
                priority: has_priority,
                key,
            },
        );
        Self::dispatch(&self.inner, scheduler);
        drop(discarded);
    }
}

impl<I, O, K> JobPool<I, O, K>
where
    I: Send + 'static,
    O: Send + 'static,
    K: Hash + Eq + Send + 'static,
{
    fn dispatch(inner: &Arc<Inner<I, O, K>>, mut scheduler: MutexGuard<Scheduler<I, bool, K>>) {
        if inner.shutdown_requested.load(Ordering::Relaxed) {
            return;
        }

        let current_num_threads = inner.pool.current_num_threads();
        let count = scheduler.admissible_count(current_num_threads);
        let mut jobs = Vec::with_capacity(count);
        while let Some(job) = scheduler.admit(current_num_threads) {
            jobs.push(job);
        }
        drop(scheduler);

        for Admitted { input, key } in jobs {
            let pool = &inner.pool;
            let inner = inner.clone();
            pool.spawn(move || {
                let output = (inner.handler)(input);
                _ = inner.out_tx.send(Completed { output, key });

                let mut scheduler = inner.scheduler.lock().unwrap();
                scheduler.complete();
                Self::dispatch(&inner, scheduler);
            });
        }
    }
}

impl<I, O, K> Drop for JobPool<I, O, K> {
    fn drop(&mut self) {
        self.inner.shutdown_requested.store(true, Ordering::Relaxed);
    }
}

struct Inner<I, O, K> {
    pool: rayon::ThreadPool,
    handler: Box<dyn Fn(I) -> O + Send + Sync>,
    out_tx: Sender<Completed<O, K>>,
    scheduler: Mutex<Scheduler<I, bool, K>>,
    shutdown_requested: AtomicBool,
}

struct SubmitOptions<K> {
    pub has_priority: bool,
    pub key: Option<K>,
}

impl<K> Default for SubmitOptions<K> {
    fn default() -> Self {
        Self {
            has_priority: false,
            key: None,
        }
    }
}

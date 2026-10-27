use super::queue::{Handle, PriorityQueue};
use enum_map::Enum;
use rustc_hash::FxHashMap;
use std::{hash::Hash, mem};

pub(super) struct Scheduler<I, P: Enum, K> {
    queue: PriorityQueue<P, Pending<I, K>>,
    keys: FxHashMap<K, KeyState<P>>,
    in_flight: usize,
}

impl<I, P: Enum, K> Scheduler<I, P, K> {
    pub(super) fn admissible_count(&self, max_in_flight: usize) -> usize {
        max_in_flight
            .saturating_sub(self.in_flight)
            .min(self.queue.len())
    }
}

impl<I, P: Enum, K: Hash + Eq> Scheduler<I, P, K> {
    pub(super) fn admit(&mut self, max_in_flight: usize) -> Option<Admitted<I, K>> {
        if self.in_flight >= max_in_flight {
            return None;
        }

        let Pending { input, key } = self.queue.pop()?;
        if let Some(key) = &key {
            *self.keys.get_mut(key).unwrap() = KeyState::Admitted;
        }
        self.in_flight += 1;
        Some(Admitted { input, key })
    }

    pub(super) fn complete(&mut self) {
        self.in_flight -= 1;
    }

    pub(super) fn finish(&mut self, key: &K) -> Option<(K, KeyState<P>)> {
        self.keys.remove_entry(key)
    }
}

impl<I, P: Enum, K: Hash + Eq + Clone> Scheduler<I, P, K> {
    pub(super) fn enqueue(
        &mut self,
        input: I,
        options: EnqueueOptions<P, K>,
    ) -> Option<Pending<I, K>> {
        let EnqueueOptions { priority, key } = options;

        let pending = Pending { input, key };

        if let Some(key) = &pending.key
            && let Some(state) = self.keys.get_mut(key)
        {
            let KeyState::Queued(handle) = state else {
                return Some(pending);
            };

            if handle.priority.to_index() < priority.to_index() {
                let replaced = self.queue.remove(handle).unwrap();
                *handle = self.queue.insert(priority, pending);
                return Some(replaced);
            }

            let queued = self.queue.get_mut(handle).unwrap();
            return Some(mem::replace(queued, pending));
        }

        let key = pending.key.clone();
        let handle = self.queue.insert(priority, pending);
        if let Some(key) = key {
            self.keys.insert(key, KeyState::Queued(handle));
        }
        None
    }
}

impl<I, P: Enum, K> Default for Scheduler<I, P, K> {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            keys: FxHashMap::default(),
            in_flight: 0,
        }
    }
}

pub(super) enum KeyState<P> {
    Queued(Handle<P>),
    Admitted,
}

pub(super) struct Pending<I, K> {
    input: I,
    key: Option<K>,
}

pub(super) struct Admitted<I, K> {
    pub(super) input: I,
    pub(super) key: Option<K>,
}

pub(super) struct Completed<O, K> {
    pub(super) output: O,
    pub(super) key: Option<K>,
}

pub(super) struct EnqueueOptions<P, K> {
    pub(super) priority: P,
    pub(super) key: Option<K>,
}

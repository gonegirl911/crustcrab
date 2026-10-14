use crate::shared::{
    enum_map::{Enum, EnumMap},
    hashlink::FxLinkedHashMap,
};
use rustc_hash::FxHashMap;
use std::{hash::Hash, mem};

pub struct Scheduler<I, K, P: Enum> {
    queues: EnumMap<P, FxLinkedHashMap<u64, Pending<I, K>>>,
    keys: FxHashMap<K, KeyState<P>>,
    next_ticket_id: u64,
    in_flight: usize,
}

impl<I, K: Hash + Eq + Clone, P: Enum> Scheduler<I, K, P> {
    pub fn enqueue(&mut self, input: I, options: EnqueueOptions<K, P>) -> Option<Pending<I, K>> {
        let EnqueueOptions { key, priority } = options;

        let pending = Pending { input, key };

        if let Some(key) = &pending.key
            && let Some(state) = self.keys.get_mut(key)
        {
            let KeyState::Pending(ticket) = state else {
                return Some(pending);
            };

            if ticket.priority.to_index() < priority.to_index() {
                let replaced = self.queues[ticket.priority].remove(&ticket.id).unwrap();
                self.queues[priority].insert(ticket.id, pending);
                ticket.priority = priority;
                return Some(replaced);
            }

            let queue = &mut self.queues[ticket.priority];
            let queued = queue.get_mut(&ticket.id).unwrap();
            return Some(mem::replace(queued, pending));
        }

        let ticket_id = self.next_ticket_id;
        self.next_ticket_id += 1;

        if let Some(key) = &pending.key {
            self.keys.insert(
                key.clone(),
                KeyState::Pending(Ticket {
                    id: ticket_id,
                    priority,
                }),
            );
        }

        let queue = &mut self.queues[priority];
        queue.insert(ticket_id, pending);
        None
    }
}

impl<I, K: Hash + Eq, P: Enum> Scheduler<I, K, P> {
    pub fn admit(&mut self, capacity: usize) -> Option<Admitted<I, K>> {
        if self.in_flight >= capacity {
            return None;
        }

        let Pending { input, key } = self.pop_next()?;
        if let Some(key) = &key {
            *self.keys.get_mut(key).unwrap() = KeyState::Admitted;
        }
        self.in_flight += 1;
        Some(Admitted { input, key })
    }

    pub fn complete(&mut self) {
        self.in_flight -= 1;
    }

    pub fn finish(&mut self, key: &K) -> Option<(K, KeyState<P>)> {
        self.keys.remove_entry(key)
    }

    fn pop_next(&mut self) -> Option<Pending<I, K>> {
        self.queues
            .values_mut()
            .rev()
            .find_map(|queue| queue.pop_front())
            .map(|(_, job)| job)
    }
}

impl<I, K, P: Enum> Default for Scheduler<I, K, P> {
    fn default() -> Self {
        Self {
            queues: Default::default(),
            next_ticket_id: 0,
            keys: Default::default(),
            in_flight: 0,
        }
    }
}

pub struct Pending<I, K> {
    input: I,
    key: Option<K>,
}

pub struct Admitted<I, K> {
    pub input: I,
    pub key: Option<K>,
}

pub struct Completed<O, K> {
    pub output: O,
    pub key: Option<K>,
}

pub enum KeyState<P> {
    Pending(Ticket<P>),
    Admitted,
}

pub struct Ticket<P> {
    id: u64,
    priority: P,
}

pub struct EnqueueOptions<K, P> {
    pub key: Option<K>,
    pub priority: P,
}

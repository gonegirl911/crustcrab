use crate::collections::hashlink::FxLinkedHashMap;
use enum_map::{Enum, EnumMap};

pub(super) struct PriorityQueue<P: Enum, T> {
    queues: EnumMap<P, FxLinkedHashMap<u64, T>>,
    len: usize,
    next_id: u64,
}

impl<P: Enum, T> PriorityQueue<P, T> {
    pub(super) fn insert(&mut self, priority: P, value: T) -> Handle<P> {
        let id = self.next_id;
        self.queues[priority].insert(id, value);
        self.len += 1;
        self.next_id += 1;
        Handle { priority, id }
    }

    pub(super) fn get_mut(&mut self, handle: &Handle<P>) -> Option<&mut T> {
        self.queues[handle.priority].get_mut(&handle.id)
    }

    pub(super) fn remove(&mut self, handle: &Handle<P>) -> Option<T> {
        let value = self.queues[handle.priority].remove(&handle.id)?;
        self.len -= 1;
        Some(value)
    }

    pub(super) fn pop(&mut self) -> Option<T> {
        self.queues
            .values_mut()
            .rev()
            .find_map(|queue| queue.pop_front())
            .inspect(|_| {
                self.len -= 1;
            })
            .map(|(_, value)| value)
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }
}

impl<P: Enum, T> Default for PriorityQueue<P, T> {
    fn default() -> Self {
        Self {
            queues: Default::default(),
            len: 0,
            next_id: 0,
        }
    }
}

pub(super) struct Handle<P> {
    pub(super) priority: P,
    pub(super) id: u64,
}

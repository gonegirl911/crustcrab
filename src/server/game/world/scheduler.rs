use super::WorldEvent;
use crate::{
    server::{
        SERVER_CONFIG,
        connection::{ConnectionEvent, ConnectionId},
        event_loop::EventHandler,
        game::player::ChunkScope,
    },
    shared::{budget::Budget, pacer::Pacer, utils},
};
use nalgebra::Point3;
use rayon::slice::ParallelSliceMut;
use rustc_hash::FxHashMap;
use std::{
    cmp::Reverse,
    mem,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct ChunkSchedulerRegistry(pub FxHashMap<ConnectionId, ChunkScheduler>);

impl ChunkSchedulerRegistry {
    pub fn client_containing(&self, points: &[Point3<i32>]) -> impl Iterator<Item = ConnectionId> {
        self.0
            .iter()
            .filter(|(_, delivery)| {
                points
                    .iter()
                    .any(|&coords| delivery.admitted.client_contains(coords))
            })
            .map(|(&id, _)| id)
    }
}

impl EventHandler<WorldEvent> for ChunkSchedulerRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &WorldEvent, (): Self::Context<'_>) {
        match *event {
            WorldEvent::JoinRequested { id, scope, .. } => {
                self.0.insert(id, ChunkScheduler::new(scope));
            }
            WorldEvent::ChunkScopeChanged { id, scope, .. } => {
                self.0.get_mut(&id).unwrap().desired = scope;
            }
            WorldEvent::ChunkBatchAcknowledged {
                id,
                chunks_per_second,
            } => {
                self.0
                    .get_mut(&id)
                    .unwrap()
                    .acknowledge_batch(chunks_per_second);
            }
            WorldEvent::Connection(ConnectionEvent::Closed(id)) => {
                self.0.remove(&id);
            }
            _ => {}
        }
    }
}

pub struct ChunkScheduler {
    admitted: ChunkScope,
    desired: ChunkScope,
    scope_pacer: Pacer,
    pending: Vec<Point3<i32>>,
    pub unacknowledged_batches: u32,
    max_unacknowledged_batches: u32,
    budget: Budget,
}

impl ChunkScheduler {
    pub fn new(scope: ChunkScope) -> Self {
        Self {
            admitted: scope,
            desired: scope,
            scope_pacer: Pacer::new(CHUNK_SCOPE_CHANGE_GAP),
            pending: Default::default(),
            unacknowledged_batches: 0,
            max_unacknowledged_batches: 1,
            budget: Budget::new(START_CHUNKS_PER_TICK),
        }
    }

    pub fn admit_scope_change(&mut self, now: Instant) -> Option<(ChunkScope, ChunkScope)> {
        if self.admitted != self.desired && self.scope_pacer.admit(now) {
            let from = mem::replace(&mut self.admitted, self.desired);
            let to = self.admitted;
            self.pending.retain(|&coords| to.client_contains(coords));
            Some((from, to))
        } else {
            None
        }
    }

    #[rustfmt::skip]
    pub fn queue<P>(&mut self, points: P, center: Point3<i32>)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        self.pending.extend(points);
        self.pending.par_sort_unstable_by_key(|&coords| Reverse(utils::distance_squared(coords, center)));
    }

    pub fn admit_batch(&mut self) -> Option<Vec<Point3<i32>>> {
        if self.pending.is_empty() || self.unacknowledged_batches >= self.max_unacknowledged_batches
        {
            return None;
        }

        let allowance = self.budget.draw();
        let size = allowance.min(self.pending.len());

        if size == 0 {
            return None;
        }

        self.unacknowledged_batches += 1;
        Some(self.pending.split_off(self.pending.len() - size))
    }

    #[rustfmt::skip]
    pub fn acknowledge_batch(&mut self, chunks_per_second: f32) {
        let ticks_per_second = SERVER_CONFIG.event_loop.ticks_per_second;
        let chunks_per_tick = chunks_per_second / ticks_per_second as f32;
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;
        self.budget.set_rate(chunks_per_tick.clamp(MIN_CHUNKS_PER_TICK, MAX_CHUNKS_PER_TICK));
    }
}

const CHUNK_SCOPE_CHANGE_GAP: Duration = Duration::from_millis(250);
const MIN_CHUNKS_PER_TICK: f32 = 1.0;
const MAX_CHUNKS_PER_TICK: f32 = 100.0;
const START_CHUNKS_PER_TICK: f32 = 50.0;
const MAX_UNACKNOWLEDGED_BATCHES: u32 = 10;

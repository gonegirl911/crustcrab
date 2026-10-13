use super::{ChunkData, WorldEvent};
use crate::{
    server::{
        connection::{ConnectionEvent, ConnectionId},
        event_loop::{EventHandler, TICKS_PER_SECOND},
        game::player::ChunkScope,
    },
    shared::{
        ema::Ema,
        flow::{budget::Budget, pacer::Pacer},
        indexmap::FxIndexSet,
        utils,
    },
};
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use std::{
    cmp::Reverse,
    mem,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct ChunkSchedulerRegistry(pub FxHashMap<ConnectionId, ChunkScheduler>);

impl ChunkSchedulerRegistry {
    pub fn demand(&self) -> usize {
        MAX_DEMAND_PER_TICK * self.0.len()
    }

    pub fn server_contains(&self, coords: Point3<i32>) -> bool {
        self.0
            .values()
            .any(|scheduler| scheduler.admitted.server_contains(coords))
    }

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
                let scheduler = self.0.get_mut(&id).unwrap();
                scheduler.desired = scope;
            }
            WorldEvent::ChunkBatchAcknowledged {
                id,
                chunks_per_second,
            } => {
                let scheduler = self.0.get_mut(&id).unwrap();
                scheduler.acknowledge_batch(chunks_per_second);
            }
            WorldEvent::Connection(ConnectionEvent::Closed(id)) => {
                self.0.remove(&id);
            }
            _ => {}
        }
    }
}

pub struct ChunkScheduler {
    pub admitted: ChunkScope,
    desired: ChunkScope,
    scope_pacer: Pacer,
    pending: FxIndexSet<Point3<i32>>,
    unacknowledged_batches: usize,
    max_unacknowledged_batches: usize,
    chunk_rate: Ema,
    budget: Budget,
}

impl ChunkScheduler {
    fn new(scope: ChunkScope) -> Self {
        Self {
            admitted: scope,
            desired: scope,
            scope_pacer: Pacer::new(CHUNK_SCOPE_CHANGE_GAP),
            pending: Default::default(),
            unacknowledged_batches: Default::default(),
            max_unacknowledged_batches: 1,
            chunk_rate: Ema::new(CHUNK_RATE_SAMPLE_WEIGHT),
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

    pub fn queue<P>(&mut self, points: P)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        let prev_len = self.pending.len();
        self.pending.extend(points);
        if prev_len == self.pending.len() {
            return;
        }
        self.pending.par_sort_unstable_by_key(|&coords| {
            Reverse(utils::distance_squared(coords, self.admitted.center))
        });
    }

    pub fn admit_batch(
        &mut self,
        mut is_settled: impl FnMut(Point3<i32>) -> bool,
    ) -> Option<(Vec<Point3<i32>>, usize)> {
        if self.unacknowledged_batches >= self.max_unacknowledged_batches {
            return None;
        }

        let allowance = self.budget.draw();

        let mut batch = Vec::with_capacity(allowance);
        while batch.len() < allowance
            && let Some(coords) = self.pending.pop_if(|&coords| is_settled(coords))
        {
            batch.push(coords);
        }
        if batch.is_empty() {
            return None;
        }

        self.unacknowledged_batches += 1;
        Some((batch, allowance))
    }

    #[rustfmt::skip]
    fn acknowledge_batch(&mut self, chunks_per_second: Option<f32>) {
        self.unacknowledged_batches = self.unacknowledged_batches.saturating_sub(1);
        self.max_unacknowledged_batches = MAX_UNACKNOWLEDGED_BATCHES;

        let Some(sample) = chunks_per_second else {
            return;
        };

        let estimate = self.chunk_rate.smooth(sample);
        let chunks_per_tick = estimate / TICKS_PER_SECOND as f32;
        self.budget.set_rate(chunks_per_tick.clamp(MIN_CHUNKS_PER_TICK, MAX_CHUNKS_PER_TICK));
    }

    pub fn max_batch_acknowledgements(span: Duration) -> usize {
        (TICKS_PER_SECOND as f32 * span.as_secs_f32()) as usize + MAX_UNACKNOWLEDGED_BATCHES
    }
}

const CHUNK_SCOPE_CHANGE_GAP: Duration = Duration::from_millis(250);
const MAX_DEMAND_PER_TICK: usize = 100;
const MAX_UNACKNOWLEDGED_BATCHES: usize = 10;
const CHUNK_RATE_SAMPLE_WEIGHT: f32 = 0.25;
const START_CHUNKS_PER_TICK: f32 = 50.0;
const MIN_CHUNKS_PER_TICK: f32 = 1.0;
const MAX_CHUNKS_PER_TICK: f32 = {
    let max_unacknowledged_bytes = 128 * 1024 * 1024;
    let chunk_data_size = size_of::<ChunkData>();
    max_unacknowledged_bytes as f32 / chunk_data_size as f32 / MAX_UNACKNOWLEDGED_BATCHES as f32
};

use super::KickReason;
use crate::{
    client::PlayerEvent,
    server::{
        connection::{ConnectionEvent, ConnectionId},
        event_loop::EventHandler,
        game::world::scheduler::ChunkScheduler,
    },
    shared::flow::{pacer::Pacer, policer::Policer},
};
use log::info;
use rustc_hash::FxHashMap;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct RateLimiterRegistry(pub FxHashMap<ConnectionId, RateLimiter>);

impl EventHandler<ConnectionEvent> for RateLimiterRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        match *event {
            ConnectionEvent::Opened(id, _) => {
                self.0.insert(id, Default::default());
            }
            ConnectionEvent::Closed(id) => {
                let mut rate_limiter = self.0.remove(&id).unwrap();

                let rate_violations = rate_limiter.rate_violations.evict(Instant::now());
                if rate_violations > 0 {
                    info!("[{id}] closed; rate violations: {rate_violations}");
                }
            }
        }
    }
}

pub struct RateLimiter {
    join_requested: Pacer,
    join_acknowledged: Pacer,
    position: Pacer,
    orientation: Pacer,
    block_action: Pacer,
    chunk_batch_acknowledged: Policer,
    rate_violations: RateViolationsTracker,
}

impl RateLimiter {
    pub fn judge(&mut self, event: &PlayerEvent, now: Instant) -> Verdict {
        let is_event_admitted = self.admit(event, now);
        if is_event_admitted {
            self.rate_violations.end();
            Verdict::Admit
        } else {
            let rate_violations = self.rate_violations.evict(now);
            let verdict = if rate_violations >= RATE_VIOLATIONS_THRESHOLD {
                Verdict::Kick(KickReason::ExcessiveRate {
                    violations: rate_violations as u32,
                })
            } else {
                Verdict::Drop
            };
            self.rate_violations.begin(now);
            verdict
        }
    }

    fn admit(&mut self, event: &PlayerEvent, now: Instant) -> bool {
        match event {
            PlayerEvent::JoinRequested { .. } => self.join_requested.admit(now),
            PlayerEvent::JoinAcknowledged => self.join_acknowledged.admit(now),
            PlayerEvent::Position { .. } => self.position.admit(now),
            PlayerEvent::Orientation { .. } => self.orientation.admit(now),
            PlayerEvent::BlockPlaced(_) | PlayerEvent::BlockDestroyed => {
                self.block_action.admit(now)
            }
            PlayerEvent::ChunkBatchAcknowledged { .. } => self.chunk_batch_acknowledged.police(now),
            PlayerEvent::KeepAlive { .. } => unreachable!(),
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            join_requested: Pacer::new(JOIN_REQUEST_GAP),
            join_acknowledged: Pacer::new(JOIN_ACKNOWLEDGEMENT_GAP),
            position: Pacer::new(POSITION_GAP),
            orientation: Pacer::new(ORIENTATION_GAP),
            block_action: Pacer::new(BLOCK_ACTION_GAP),
            chunk_batch_acknowledged: Policer::new(
                CHUNK_BATCH_ACKNOWLEDGEMENT_WINDOW,
                ChunkScheduler::max_batch_acknowledgements(CHUNK_BATCH_ACKNOWLEDGEMENT_WINDOW),
            ),
            rate_violations: Default::default(),
        }
    }
}

#[derive(Default)]
struct RateViolationsTracker {
    bursts: VecDeque<Instant>,
    in_violation: bool,
}

impl RateViolationsTracker {
    fn begin(&mut self, now: Instant) {
        if !self.in_violation {
            self.in_violation = true;
            self.bursts.push_back(now);
        }
    }

    fn end(&mut self) {
        self.in_violation = false;
    }

    fn evict(&mut self, now: Instant) -> usize {
        while let Some(&began_at) = self.bursts.front()
            && now.duration_since(began_at) > RATE_VIOLATION_WINDOW
        {
            self.bursts.pop_front();
        }

        self.bursts.len()
    }
}

pub enum Verdict {
    Admit,
    Drop,
    Kick(KickReason),
}

const JOIN_REQUEST_GAP: Duration = Duration::from_secs(1);
const JOIN_ACKNOWLEDGEMENT_GAP: Duration = Duration::from_secs(1);
const POSITION_GAP: Duration = Duration::from_millis(2);
const ORIENTATION_GAP: Duration = Duration::from_millis(2);
const BLOCK_ACTION_GAP: Duration = Duration::from_millis(16);
const CHUNK_BATCH_ACKNOWLEDGEMENT_WINDOW: Duration = Duration::from_secs(1);

const RATE_VIOLATION_WINDOW: Duration = Duration::from_secs(60);
const RATE_VIOLATIONS_THRESHOLD: usize = 360;

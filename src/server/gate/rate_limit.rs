use crate::{
    client::PlayerEvent,
    server::{
        connection::{ConnectionEvent, ConnectionId},
        event_loop::EventHandler,
    },
    shared::{pacer::Pacer, utils},
};
use log::{info, warn};
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

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
                let rate_limiter = self.0.remove(&id).unwrap();

                if rate_limiter.invalid_events > 0 {
                    warn!(
                        "[{id:?}] closed; invalid events: {}, rate violations: {}",
                        rate_limiter.invalid_events, rate_limiter.rate_violations.count,
                    );
                } else if rate_limiter.rate_violations.count > 0 {
                    info!(
                        "[{id:?}] closed; rate violations: {}",
                        rate_limiter.rate_violations.count,
                    );
                }
            }
        }
    }
}

#[derive(Default)]
pub struct RateLimiter {
    join_requested: Pacer,
    position: Pacer,
    orientation: Pacer,
    block_action: Pacer,
    chunk_scope: ChunkScopeTracker,
    rate_violations: RateViolationsTracker,
    invalid_events: u32,
    was_kicked: bool,
}

impl RateLimiter {
    pub fn judge(&mut self, event: &PlayerEvent, now: Instant) -> Verdict {
        let is_event_admitted = self.admit(event, now);

        if !is_event_admitted {
            self.rate_violations.begin();
        }

        let verdict = self.verdict(is_event_admitted);
        if matches!(verdict, Verdict::Kick(_)) {
            self.was_kicked = true;
        }
        verdict
    }

    pub fn record_invalid_event(&mut self) -> Verdict {
        self.invalid_events += 1;

        let verdict = self.verdict(false);
        if matches!(verdict, Verdict::Kick(_)) {
            self.was_kicked = true;
        }
        verdict
    }

    fn admit(&mut self, event: &PlayerEvent, now: Instant) -> bool {
        match event {
            PlayerEvent::JoinRequested { .. } => {
                if !self.join_requested.is_due(JOIN_REQUEST_GAP, now) {
                    return false;
                }

                self.join_requested.stamp(now);
                self.chunk_scope = Default::default();
                self.rate_violations.end();
                true
            }
            PlayerEvent::Position { origin } => {
                if !self.position.is_due(POSITION_GAP, now) {
                    return false;
                }

                if !self.chunk_scope.admit(utils::chunk_coords(*origin), now) {
                    return false;
                }

                self.position.stamp(now);
                self.rate_violations.end();
                true
            }
            PlayerEvent::Orientation { .. } => {
                if !self.orientation.is_due(ORIENTATION_GAP, now) {
                    return false;
                }

                self.orientation.stamp(now);
                self.rate_violations.end();
                true
            }
            PlayerEvent::BlockPlaced(_) | PlayerEvent::BlockDestroyed => {
                if !self.block_action.is_due(BLOCK_ACTION_GAP, now) {
                    return false;
                }

                self.block_action.stamp(now);
                self.rate_violations.end();
                true
            }
            _ => true,
        }
    }

    fn verdict(&self, is_event_admitted: bool) -> Verdict {
        if self.was_kicked {
            Verdict::Drop
        } else if is_event_admitted {
            Verdict::Admit
        } else if self.invalid_events >= INVALID_EVENTS_THRESHOLD {
            Verdict::Kick(KickReason::InvalidEvents {
                count: self.invalid_events,
            })
        } else if self.rate_violations.count >= RATE_VIOLATIONS_THRESHOLD {
            Verdict::Kick(KickReason::ExcessiveRate {
                violations: self.rate_violations.count,
            })
        } else {
            Verdict::Drop
        }
    }
}

#[derive(Default)]
struct ChunkScopeTracker {
    pacer: Pacer,
    last_center: Option<Point3<i32>>,
}

impl ChunkScopeTracker {
    fn admit(&mut self, center: Point3<i32>, now: Instant) -> bool {
        if self.last_center == Some(center) {
            return true;
        }

        if !self.pacer.is_due(CHUNK_SCOPE_CHANGE_GAP, now) {
            return false;
        }

        self.pacer.stamp(now);
        self.last_center = Some(center);
        true
    }
}

#[derive(Default)]
struct RateViolationsTracker {
    count: u32,
    in_violation: bool,
}

impl RateViolationsTracker {
    fn begin(&mut self) {
        self.count += !self.in_violation as u32;
        self.in_violation = true;
    }

    fn end(&mut self) {
        self.in_violation = false;
    }
}

pub enum Verdict {
    Admit,
    Drop,
    Kick(KickReason),
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum KickReason {
    #[error("too many invalid events ({count} rejected)")]
    InvalidEvents { count: u32 },
    #[error("excessive event rate ({violations} violations)")]
    ExcessiveRate { violations: u32 },
}

const JOIN_REQUEST_GAP: Duration = Duration::from_secs(1);
const POSITION_GAP: Duration = Duration::from_millis(2);
const ORIENTATION_GAP: Duration = Duration::from_millis(2);
const BLOCK_ACTION_GAP: Duration = Duration::from_millis(20);
const CHUNK_SCOPE_CHANGE_GAP: Duration = Duration::from_millis(250);

const INVALID_EVENTS_THRESHOLD: u32 = 4;
const RATE_VIOLATIONS_THRESHOLD: u32 = 2000;

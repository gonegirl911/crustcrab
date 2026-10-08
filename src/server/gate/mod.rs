pub mod keep_alive;
pub mod rate_limit;
pub mod sanitization;
pub mod session;

use super::{
    connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
    event_loop::{Event, EventHandler},
};
use crate::{client::PlayerEvent, server::ControlEvent};
use keep_alive::KeepAliveRegistry;
use log::warn;
use rate_limit::{RateLimiterRegistry, Verdict};
use serde::{Deserialize, Serialize};
use session::SessionRegistry;
use std::time::Instant;
use thiserror::Error;

#[derive(Default)]
pub struct Gate {
    sessions: SessionRegistry,
    keep_alives: KeepAliveRegistry,
    rate_limiters: RateLimiterRegistry,
}

impl Gate {
    pub fn admit(
        &mut self,
        id: ConnectionId,
        event: PlayerEvent,
        connections: &ConnectionRegistry,
    ) -> Option<Event> {
        if !connections.0.contains_key(&id) {
            return None;
        }

        if self.sessions.is_kicked(id) {
            return None;
        }

        if let PlayerEvent::KeepAlive { tag } = event {
            return if self.keep_alives.acknowledge(id, tag) {
                None
            } else {
                self.kick(connections, id, KickReason::InvalidEvent)
            };
        }

        let Some(event) = sanitization::sanitize(event) else {
            return self.kick(connections, id, KickReason::InvalidEvent);
        };

        let rate_limiter = self.rate_limiters.0.get_mut(&id).unwrap();
        match rate_limiter.judge(&event, Instant::now()) {
            Verdict::Admit => {}
            Verdict::Drop => {
                return None;
            }
            Verdict::Kick(reason) => {
                return self.kick(connections, id, reason);
            }
        }

        self.sessions.admit(id, event)
    }

    pub fn keep_alive(&mut self, connections: &ConnectionRegistry, now: Instant) {
        for id in self.keep_alives.sweep(connections, now) {
            if !self.sessions.is_kicked(id) {
                self.kick(connections, id, KickReason::TimedOut);
            }
        }
    }

    fn kick(
        &mut self,
        connections: &ConnectionRegistry,
        id: ConnectionId,
        reason: KickReason,
    ) -> Option<Event> {
        self.sessions.kick(id);
        warn!("[{id}] kicked: {reason}");
        connections.one(id).send(ControlEvent::Kicked { reason });
        None
    }
}

impl EventHandler<ConnectionEvent> for Gate {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        self.sessions.handle(event, ());
        self.keep_alives.handle(event, ());
        self.rate_limiters.handle(event, ());
    }
}

#[derive(Clone, Debug, Error, Serialize, Deserialize)]
pub enum KickReason {
    #[error("invalid event")]
    InvalidEvent,
    #[error("excessive event rate ({violations} violations)")]
    ExcessiveRate { violations: u32 },
    #[error("connection timed out")]
    TimedOut,
}

pub mod rate_limit;
pub mod sanitization;
pub mod session;

use super::{
    connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
    event_loop::{Event, EventHandler},
};
use crate::{client::PlayerEvent, server::ControlEvent};
use log::warn;
use rate_limit::{KickReason, RateLimiterRegistry, Verdict};
use session::SessionRegistry;
use std::time::Instant;

#[derive(Default)]
pub struct Gate {
    rate_limiters: RateLimiterRegistry,
    sessions: SessionRegistry,
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

        let rate_limiter = self.rate_limiters.0.get_mut(&id).unwrap();

        let Some(event) = sanitization::sanitize(event) else {
            return if let Verdict::Kick(reason) = rate_limiter.record_invalid_event() {
                Self::kick(connections, id, reason)
            } else {
                None
            };
        };

        match rate_limiter.judge(&event, Instant::now()) {
            Verdict::Admit => {}
            Verdict::Drop => {
                return None;
            }
            Verdict::Kick(reason) => {
                return Self::kick(connections, id, reason);
            }
        }

        self.sessions.admit(id, event)
    }

    fn kick(
        connections: &ConnectionRegistry,
        id: ConnectionId,
        reason: KickReason,
    ) -> Option<Event> {
        warn!("[{id:?}] kicked: {reason}");
        connections.one(id).send(ControlEvent::Kicked { reason });
        None
    }
}

impl EventHandler<ConnectionEvent> for Gate {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        self.rate_limiters.handle(event, ());
        self.sessions.handle(event, ());
    }
}

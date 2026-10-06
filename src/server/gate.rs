use super::{
    connection::{ConnectionEvent, ConnectionId, ConnectionRegistry},
    event_loop::{Event, EventHandler},
    session::SessionRegistry,
};
use crate::client::PlayerEvent;

#[derive(Default)]
pub struct Gate {
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

        self.sessions.admit(id, event)
    }
}

impl EventHandler<ConnectionEvent> for Gate {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        self.sessions.handle(event, ());
    }
}

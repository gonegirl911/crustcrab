use super::{
    connection::{ConnectionEvent, ConnectionId},
    event_loop::{Event, EventHandler},
};
use crate::client::PlayerEvent;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct SessionRegistry(FxHashMap<ConnectionId, Session>);

impl SessionRegistry {
    pub fn admit(&mut self, id: ConnectionId, event: PlayerEvent) -> Option<Event> {
        match self.0.entry(id) {
            Entry::Occupied(mut entry) => {
                let session = entry.get_mut();
                match event {
                    PlayerEvent::JoinRequested { .. } if !matches!(session, Session::Joining) => {
                        *session = Session::Joining;
                        Some(Event::Player(id, event))
                    }
                    PlayerEvent::JoinAcknowledged => {
                        *session = Session::Joined;
                        Some(Event::Player(id, event))
                    }
                    _ if matches!(session, Session::Joined) => Some(Event::Player(id, event)),
                    _ => None,
                }
            }
            Entry::Vacant(entry) => {
                if let PlayerEvent::JoinRequested { .. } = event {
                    entry.insert(Session::Joining);
                    Some(Event::Player(id, event))
                } else {
                    None
                }
            }
        }
    }
}

impl EventHandler<ConnectionEvent> for SessionRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        if let ConnectionEvent::Closed(id) = event {
            self.0.remove(id);
        }
    }
}

#[derive(Clone, Copy)]
pub enum Session {
    Joining,
    Joined,
}

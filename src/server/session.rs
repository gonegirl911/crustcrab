use super::{
    connection::{ConnectionId, ConnectionRegistry},
    event_loop::Event,
};
use crate::client::PlayerEvent;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct SessionRegistry(pub FxHashMap<ConnectionId, Session>);

impl SessionRegistry {
    pub fn admit(
        &mut self,
        id: ConnectionId,
        event: PlayerEvent,
        connections: &ConnectionRegistry,
    ) -> Option<Event> {
        if !connections.0.contains_key(&id) {
            return None;
        }

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

#[derive(Clone, Copy)]
pub enum Session {
    Joining,
    Joined,
}

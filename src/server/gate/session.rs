use crate::{
    client::PlayerEvent,
    server::{
        connection::{ConnectionEvent, ConnectionId},
        event_loop::{Event, EventHandler},
    },
};
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct SessionRegistry(FxHashMap<ConnectionId, Session>);

impl SessionRegistry {
    pub fn admit(&mut self, id: ConnectionId, event: PlayerEvent) -> Option<Event> {
        let session = self.0.get_mut(&id).unwrap();
        match (*session, &event) {
            (Session::Opened, PlayerEvent::JoinRequested { .. }) => {
                *session = Session::Joining;
                Some(Event::Player(id, event))
            }
            (Session::Joining, PlayerEvent::JoinAcknowledged) => {
                *session = Session::Joined;
                None
            }
            (Session::Joined, PlayerEvent::JoinRequested { .. }) => {
                *session = Session::Joining;
                Some(Event::Player(id, event))
            }
            (Session::Joined, PlayerEvent::JoinAcknowledged) => None,
            (Session::Joined, _) => Some(Event::Player(id, event)),
            _ => None,
        }
    }

    pub fn is_kicked(&self, id: ConnectionId) -> bool {
        let session = self.0[&id];
        matches!(session, Session::Kicked)
    }

    pub fn kick(&mut self, id: ConnectionId) {
        let session = self.0.get_mut(&id).unwrap();
        *session = Session::Kicked;
    }
}

impl EventHandler<ConnectionEvent> for SessionRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        match *event {
            ConnectionEvent::Opened(id, _) => {
                self.0.insert(id, Session::Opened);
            }
            ConnectionEvent::Closed(id) => {
                self.0.remove(&id);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Session {
    Opened,
    Joining,
    Joined,
    Kicked,
}

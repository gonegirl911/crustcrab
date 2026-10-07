use super::{ChunkEvent, ControlEvent, event_loop::EventHandler};
use crossbeam_channel::{SendError, Sender};
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::{
    fmt::{self, Display, Formatter},
    iter,
};
use uuid::Uuid;

#[derive(Default)]
pub struct ConnectionRegistry(pub FxHashMap<ConnectionId, Connection>);

impl ConnectionRegistry {
    pub fn one<'a>(&'a self, id: ConnectionId) -> RecipientList<'a> {
        RecipientList::One(&self.0[&id])
    }

    pub fn many<'a, I>(&'a self, ids: I) -> RecipientList<'a>
    where
        I: IntoIterator<Item = ConnectionId>,
    {
        RecipientList::Many(ids.into_iter().map(|id| &self.0[&id]).collect())
    }

    pub fn all<'a>(&'a self) -> RecipientList<'a> {
        RecipientList::All(self)
    }
}

impl EventHandler<ConnectionEvent> for ConnectionRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &ConnectionEvent, (): Self::Context<'_>) {
        match event {
            ConnectionEvent::Opened(id, connection) => {
                self.0.insert(*id, connection.clone());
            }
            ConnectionEvent::Closed(id) => {
                self.0.remove(id);
            }
        }
    }
}

pub enum RecipientList<'a> {
    One(&'a Connection),
    Many(Vec<&'a Connection>),
    All(&'a ConnectionRegistry),
}

impl<'a> RecipientList<'a> {
    pub fn send<E: Outbound + Clone>(&self, event: E) {
        match self {
            Self::One(connection) => {
                _ = connection.send(event);
            }
            Self::Many(connections) => {
                let len = connections.len();
                for (connection, event) in connections.iter().zip(iter::repeat_n(event, len)) {
                    _ = connection.send(event);
                }
            }
            Self::All(connections) => {
                let len = connections.0.len();
                for (connection, event) in connections.0.values().zip(iter::repeat_n(event, len)) {
                    _ = connection.send(event);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Connection {
    pub control_tx: Sender<ControlEvent>,
    pub chunk_tx: Sender<ChunkEvent>,
}

impl Connection {
    fn send<E: Outbound + Clone>(&self, event: E) -> Result<(), SendError<E>> {
        event.send(self)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct ConnectionId(Uuid);

impl ConnectionId {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Display for ConnectionId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone)]
pub enum ConnectionEvent {
    Opened(ConnectionId, Connection),
    Closed(ConnectionId),
}

pub trait Outbound: Sized {
    fn send(self, connection: &Connection) -> Result<(), SendError<Self>>;
}

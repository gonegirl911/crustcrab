use super::WorldEvent;
use crate::{
    server::{
        connection::{ConnectionEvent, ConnectionId},
        event_loop::EventHandler,
    },
    shared::ray::BlockIntersection,
};
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ViewerRegistry(pub FxHashMap<ConnectionId, Viewer>);

impl EventHandler<WorldEvent> for ViewerRegistry {
    type Context<'a> = ();

    fn handle(&mut self, event: &WorldEvent, (): Self::Context<'_>) {
        match *event {
            WorldEvent::JoinRequested { id, .. } => {
                self.0.insert(id, Viewer { hover: None });
            }
            WorldEvent::Connection(ConnectionEvent::Closed(id)) => {
                self.0.remove(&id);
            }
            _ => {}
        }
    }
}

pub struct Viewer {
    pub hover: Option<BlockIntersection>,
}

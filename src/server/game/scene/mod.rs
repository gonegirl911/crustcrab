pub mod clock;
pub mod world;

use self::{clock::Clock, world::World};
use super::player::Player;
use crate::server::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;

#[derive(Default)]
pub struct Scene {
    clock: Clock,
    world: World,
}

impl EventHandler<Event> for Scene {
    type Context<'a> = (Sender<ServerEvent>, &'a Player);

    fn handle(&mut self, event: &Event, (server_tx, player): Self::Context<'_>) {
        self.clock.handle(event, server_tx.clone());
        self.world.handle(event, (server_tx, player));
    }
}

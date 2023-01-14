pub mod clock;
pub mod player;
pub mod world;

use self::{clock::Clock, player::Player, world::World};
use super::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;

#[derive(Default)]
pub struct Scene {
    player: Player,
    clock: Clock,
    world: World,
}

impl EventHandler<Event> for Scene {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.clock.handle(event, server_tx.clone());
        self.world.handle(event, (server_tx, &self.player));
    }
}

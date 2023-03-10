pub mod player;
pub mod world;

use self::{player::Player, world::World};
use super::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;

#[derive(Default)]
pub struct Game {
    player: Player,
    world: World,
}

impl EventHandler<Event> for Game {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.world.handle(event, (server_tx, &self.player));
    }
}

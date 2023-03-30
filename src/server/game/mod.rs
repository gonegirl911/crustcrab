pub mod clock;
pub mod player;
pub mod world;

use self::{clock::Clock, player::Player, world::World};
use super::{
    event_loop::{Event, EventHandler},
    ServerEvent, ServerSettings,
};
use flume::Sender;

pub struct Game {
    player: Player,
    clock: Clock,
    world: World,
}

impl Game {
    pub fn new(settings: &ServerSettings) -> Self {
        Self {
            player: Default::default(),
            clock: Default::default(),
            world: World::new(settings),
        }
    }
}

impl EventHandler<Event> for Game {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.clock.handle(event, server_tx.clone());
        self.world.handle(event, (server_tx, &self.player));
    }
}

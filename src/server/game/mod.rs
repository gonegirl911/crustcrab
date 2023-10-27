pub mod clock;
pub mod player;
pub mod world;

use self::{
    clock::Clock,
    player::Player,
    world::{World, WorldEvent},
};
use super::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;
use std::thread;

pub struct Game {
    player: Player,
    clock: Clock,
    world_tx: Sender<(WorldEvent, Sender<ServerEvent>)>,
}

impl Default for Game {
    fn default() -> Self {
        let player = Default::default();
        let clock = Default::default();
        let mut world = World::default();
        let (world_tx, world_rx) = flume::unbounded();

        thread::spawn(move || {
            for (event, server_tx) in world_rx {
                world.handle(&event, &server_tx);
            }
        });

        Self {
            player,
            clock,
            world_tx,
        }
    }
}

impl EventHandler<Event> for Game {
    type Context<'a> = &'a Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.clock.handle(event, server_tx);

        if let Some(event) = WorldEvent::new(event, &self.player) {
            self.world_tx
                .send((event, server_tx.clone()))
                .unwrap_or_else(|_| unreachable!());
        }
    }
}

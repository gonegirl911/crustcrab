pub mod clock;
pub mod player;
pub mod world;

use self::{
    clock::Clock,
    player::Player,
    world::{World, WorldEvent},
};
use super::event_loop::{Event, EventHandler};
use crate::client::event_loop::EventLoopProxy;
use flume::Sender;
use std::thread;

pub struct Game {
    player: Player,
    clock: Clock,
    world_tx: Sender<(WorldEvent, EventLoopProxy)>,
}

impl Default for Game {
    fn default() -> Self {
        let player = Default::default();
        let clock = Default::default();
        let (world_tx, world_rx) = flume::unbounded();

        thread::spawn(move || {
            let mut world = World::default();
            for (event, proxy) in world_rx {
                world.handle(&event, &proxy);
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
    type Context<'a> = &'a EventLoopProxy;

    fn handle(&mut self, event: &Event, proxy: Self::Context<'_>) {
        self.player.handle(event, ());
        self.clock.handle(event, proxy);

        if let Some(event) = WorldEvent::new(event, &self.player) {
            self.world_tx
                .send((event, proxy.clone()))
                .unwrap_or_else(|_| unreachable!());
        }
    }
}

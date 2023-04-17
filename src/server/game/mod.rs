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
    ServerEvent, ServerSettings,
};
use flume::Sender;
use std::thread;

pub struct Game {
    player: Player,
    clock: Clock,
    world_tx: Sender<(WorldEvent, Sender<ServerEvent>)>,
}

impl Game {
    pub fn new(settings: &ServerSettings) -> Self {
        let player = Default::default();
        let clock = Default::default();
        let mut world = World::new(settings);
        let (world_tx, world_rx) = flume::unbounded();

        thread::spawn(move || {
            for (event, server_tx) in world_rx {
                world.handle(&event, server_tx);
            }
        });

        Self {
            player,
            clock,
            world_tx,
        }
    }

    fn send_world_events<I>(&self, events: I, server_tx: Sender<ServerEvent>)
    where
        I: IntoIterator<Item = WorldEvent>,
    {
        for event in events {
            self.world_tx
                .send((event, server_tx.clone()))
                .unwrap_or_else(|_| unreachable!());
        }
    }
}

impl EventHandler<Event> for Game {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.clock.handle(event, server_tx.clone());
        self.send_world_events(WorldEvent::new(event, &self.player), server_tx);
    }
}

pub mod clock;
pub mod player;
pub mod world;

use super::{
    ChunkEvent, Connection, ControlEvent,
    event_loop::{Event, EventHandler},
};
use clock::Clock;
use crossbeam_channel::Sender;
use player::Player;
use std::thread;
use world::{World, WorldEvent};

pub struct Game {
    player: Player,
    clock: Clock,
    world_tx: Sender<(WorldEvent, Sender<ControlEvent>, Sender<ChunkEvent>)>,
}

impl Default for Game {
    fn default() -> Self {
        let player = Default::default();
        let clock = Default::default();
        let (world_tx, world_rx) = crossbeam_channel::unbounded();

        thread::spawn(move || {
            let mut world = World::default();
            for (event, control_tx, chunk_tx) in world_rx {
                world.handle(&event, (&control_tx, &chunk_tx));
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
    type Context<'a> = &'a Connection;

    fn handle(
        &mut self,
        event: &Event,
        Connection {
            control_tx,
            chunk_tx,
        }: Self::Context<'_>,
    ) {
        self.player.handle(event, control_tx);
        self.clock.handle(event, control_tx);

        if let Some(event) = WorldEvent::new(event, &self.player) {
            self.world_tx
                .send((event, control_tx.clone(), chunk_tx.clone()))
                .unwrap();
        }
    }
}

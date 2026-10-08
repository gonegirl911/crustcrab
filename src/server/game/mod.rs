pub mod block;
pub mod chunk;
pub mod clock;
pub mod coords;
pub mod player;
pub mod world;

use super::{
    actor::Actor,
    connection::ConnectionRegistry,
    event_loop::{Event, EventHandler},
};
use clock::Clock;
use player::PlayerRegistry;
use world::{World, WorldEvent};

pub struct Game {
    clock: Clock,
    players: PlayerRegistry,
    world: Actor<WorldEvent>,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            clock: Default::default(),
            players: Default::default(),
            world: Actor::spawn(World::default()),
        }
    }
}

impl EventHandler<Event> for Game {
    type Context<'a> = &'a ConnectionRegistry;

    fn handle(&mut self, event: &Event, connections: Self::Context<'_>) {
        self.clock.handle(event, connections);
        self.players.handle(event, connections);

        if let Event::Connection(event) = event {
            self.world.forward(event);
        }

        let player = if let Event::Player(id, _) = event {
            self.players.0.get(id)
        } else {
            None
        };

        if let Some(event) = WorldEvent::new(event, player) {
            self.world.send(event);
        }
    }
}

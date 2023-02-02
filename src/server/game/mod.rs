pub mod player;
pub mod scene;

use self::{player::Player, scene::Scene};
use super::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;

#[derive(Default)]
pub struct Game {
    player: Player,
    scene: Scene,
}

impl EventHandler<Event> for Game {
    type Context<'a> = Sender<ServerEvent>;

    fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
        self.player.handle(event, ());
        self.scene.handle(event, (server_tx, &self.player));
    }
}

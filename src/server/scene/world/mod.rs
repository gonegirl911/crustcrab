pub mod block;
pub mod chunk;
pub mod generator;

use self::chunk::{ChunkMap, ChunkMapEvent};
use super::player::Player;
use crate::server::{
    event_loop::{Event, EventHandler},
    ServerEvent,
};
use flume::Sender;
use std::thread;

pub struct World {
    chunks_tx: Sender<(ChunkMapEvent, Sender<ServerEvent>)>,
}

impl Default for World {
    fn default() -> Self {
        let (chunks_tx, chunks_rx) = flume::unbounded();

        thread::spawn(move || {
            let mut chunks = ChunkMap::default();
            for (event, server_tx) in chunks_rx {
                chunks.handle(&event, server_tx);
            }
        });

        Self { chunks_tx }
    }
}

impl EventHandler<Event> for World {
    type Context<'a> = (Sender<ServerEvent>, &'a Player);

    fn handle(&mut self, event: &Event, (server_tx, player): Self::Context<'_>) {
        if let Some(event) = ChunkMapEvent::new(event, player) {
            self.chunks_tx
                .send((event, server_tx))
                .unwrap_or_else(|_| unreachable!());
        }
    }
}

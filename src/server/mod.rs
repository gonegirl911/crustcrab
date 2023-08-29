pub mod event_loop;
pub mod game;
pub mod ticker;

use self::{
    event_loop::{EventLoop, EventLoopConfig},
    game::{
        clock::{ClockState, Time},
        player::PlayerConfig,
        world::{BlockHoverData, ChunkData},
        Game,
    },
};
use crate::{
    client::ClientEvent,
    server::event_loop::{Event, EventHandler},
};
use flume::{Receiver, Sender};
use nalgebra::Point3;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs, sync::Arc};

pub struct Server {
    event_loop: EventLoop,
    game: Game,
}

impl Server {
    pub fn new(server_tx: Sender<ServerEvent>, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(server_tx, client_rx),
            game: Default::default(),
        }
    }

    pub fn run(self) -> ! {
        struct MiniServer {
            game: Game,
        }

        impl EventHandler<Event> for MiniServer {
            type Context<'a> = Sender<ServerEvent>;

            fn handle(&mut self, event: &Event, server_tx: Self::Context<'_>) {
                self.game.handle(event, server_tx);
            }
        }

        self.event_loop.run(MiniServer { game: self.game })
    }
}

#[derive(Deserialize)]
pub struct ServerConfig {
    event_loop: EventLoopConfig,
    player: PlayerConfig,
    clock: ClockState,
}

pub static SERVER_CONFIG: Lazy<ServerConfig> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/config/server.toml").expect("file should exist"))
        .expect("file should be valid")
});

pub enum ServerEvent {
    TimeUpdated(Time),
    ChunkLoaded {
        coords: Point3<i32>,
        data: Arc<ChunkData>,
        is_important: bool,
    },
    ChunkUnloaded {
        coords: Point3<i32>,
    },
    ChunkUpdated {
        coords: Point3<i32>,
        data: Arc<ChunkData>,
        is_important: bool,
    },
    BlockHovered(Option<BlockHoverData>),
}

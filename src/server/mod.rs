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
use crate::client::{event_loop::EventLoopProxy, ClientEvent};
use flume::Receiver;
use nalgebra::Point3;
use serde::Deserialize;
use std::{
    fs,
    sync::{Arc, LazyLock},
};

pub struct Server {
    event_loop: EventLoop,
    game: Game,
}

impl Server {
    pub fn new(proxy: EventLoopProxy, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(proxy, client_rx),
            game: Default::default(),
        }
    }

    pub fn run(self) {
        self.event_loop.run(self.game);
    }
}

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

#[derive(Deserialize)]
struct ServerConfig {
    event_loop: EventLoopConfig,
    player: PlayerConfig,
    clock: ClockState,
}

static SERVER_CONFIG: LazyLock<ServerConfig> = LazyLock::new(|| {
    toml::from_str(&fs::read_to_string("assets/config/server.toml").expect("file should exist"))
        .expect("file should be valid")
});

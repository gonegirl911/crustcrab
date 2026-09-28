pub(crate) mod event_loop;
pub(crate) mod game;
pub(crate) mod ticker;

use crate::{client::PlayerEvent, shared::toml};
use crossbeam_channel::{Receiver, Sender};
use event_loop::{EventLoop, EventLoopConfig};
use game::{
    Game,
    clock::ClockConfig,
    player::PlayerConfig,
    world::{BlockHoverData, ChunkData, block::Block},
};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, OnceLock};

pub struct Server {
    event_loop: EventLoop,
}

impl Server {
    pub fn new(connection_rx: Receiver<Connection>, player_rx: Receiver<PlayerEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(connection_rx, player_rx),
        }
    }

    pub fn run(&mut self) {
        self.event_loop.run(Game::default());
    }
}

#[derive(Clone)]
pub struct Connection {
    pub control_tx: Sender<ControlEvent>,
    pub chunk_tx: Sender<ChunkEvent>,
}

impl Connection {
    pub fn closed() -> Self {
        static CLOSED: OnceLock<Connection> = OnceLock::new();

        CLOSED
            .get_or_init(|| {
                let (control_tx, _) = crossbeam_channel::unbounded();
                let (chunk_tx, _) = crossbeam_channel::unbounded();
                Self {
                    control_tx,
                    chunk_tx,
                }
            })
            .clone()
    }
}

#[derive(Serialize, Deserialize)]
pub enum ControlEvent {
    PlayerInitialized {
        origin: Point3<f64>,
        dir: Vector3<f32>,
        speed: f64,
        inventory: Arc<[Block]>,
    },
    TimeUpdated {
        ticks: u16,
    },
    BlockHovered(Option<BlockHoverData>),
}

#[derive(Serialize, Deserialize)]
pub enum ChunkEvent {
    Loaded(Arc<ChunkData>),
    Unloaded { coords: Point3<i32> },
    Updated(Arc<ChunkData>),
    BatchStarted,
    BatchEnded,
}

#[derive(Deserialize)]
pub struct ServerConfig {
    pub event_loop: EventLoopConfig,
    player: PlayerConfig,
    pub clock: ClockConfig,
}

pub static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| toml::deserialize("assets/config/server.toml"));

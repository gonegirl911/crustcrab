pub(crate) mod actor;
pub mod connection;
pub(crate) mod event_loop;
pub(crate) mod game;
pub mod net;
pub(crate) mod session;
pub(crate) mod ticker;

use crate::{client::PlayerEvent, shared::toml};
use connection::{Connection, ConnectionEvent, ConnectionId, Outbound};
use crossbeam_channel::{Receiver, SendError};
use event_loop::{EventLoop, EventLoopConfig};
use game::{
    Game,
    clock::{ClockConfig, DayCycle},
    player::PlayerConfig,
    world::{BlockHoverData, ChunkData, block::Block},
};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

pub struct Server {
    event_loop: EventLoop,
}

impl Server {
    pub fn new(
        connection_rx: Receiver<ConnectionEvent>,
        player_rx: Receiver<(ConnectionId, PlayerEvent)>,
    ) -> Self {
        Self {
            event_loop: EventLoop::new(connection_rx, player_rx),
        }
    }

    pub fn run(&mut self) {
        self.event_loop.run(Game::default());
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ControlEvent {
    TimeInitialized {
        ticks_per_second: u32,
        cycle: DayCycle,
    },
    PlayerInitialized {
        origin: Point3<f64>,
        dir: Vector3<f32>,
        speed: f64,
        inventory: Arc<[Block]>,
    },
    JoinFinished,
    TimeUpdated {
        ticks: u16,
    },
    BlockHovered(Option<Arc<BlockHoverData>>),
}

impl Outbound for ControlEvent {
    fn send(self, Connection { control_tx, .. }: &Connection) -> Result<(), SendError<Self>> {
        control_tx.send(self)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ChunkEvent {
    Loaded(Arc<ChunkData>),
    Unloaded { coords: Point3<i32> },
    Updated(Arc<ChunkData>),
    BatchStarted,
    BatchEnded,
}

impl Outbound for ChunkEvent {
    fn send(self, Connection { chunk_tx, .. }: &Connection) -> Result<(), SendError<Self>> {
        chunk_tx.send(self)
    }
}

#[derive(Deserialize)]
struct ServerConfig {
    event_loop: EventLoopConfig,
    player: PlayerConfig,
    clock: ClockConfig,
}

static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| toml::deserialize("assets/config/server.toml"));

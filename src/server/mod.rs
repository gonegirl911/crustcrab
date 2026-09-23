pub(crate) mod event_loop;
pub(crate) mod game;
pub(crate) mod ticker;

use crate::{client::ClientEvent, shared::toml};
use crossbeam_channel::{Receiver, SendError, Sender};
use event_loop::{EventLoop, EventLoopConfig};
use game::{
    Game,
    clock::{ClockConfig, Time},
    player::PlayerConfig,
    world::{BlockHoverData, ChunkData, block::Block},
};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use uuid::Uuid;

pub struct Server {
    event_loop: EventLoop,
}

impl Server {
    pub fn new(server_tx: ServerSender, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(server_tx, client_rx),
        }
    }

    pub fn run(&mut self) {
        self.event_loop.run(Game::default());
    }
}

#[derive(Serialize, Deserialize)]
pub enum ServerEvent {
    PlayerInitialized {
        origin: Point3<f64>,
        dir: Vector3<f32>,
        speed: f64,
        inventory: Arc<[Block]>,
    },
    TimeUpdated(Time),
    ChunkLoaded {
        data: Arc<ChunkData>,
        group_id: Option<GroupId>,
    },
    ChunkUnloaded {
        coords: Point3<i32>,
        group_id: Option<GroupId>,
    },
    ChunkUpdated {
        data: Arc<ChunkData>,
        group_id: Option<GroupId>,
    },
    BlockHovered(Option<BlockHoverData>),
    #[serde(skip)]
    ClientDisconnected,
}

impl ServerEvent {
    fn has_priority(&self) -> bool {
        !matches!(
            self,
            Self::ChunkLoaded { .. } | Self::ChunkUnloaded { .. } | Self::ChunkUpdated { .. }
        )
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct GroupId {
    pub id: Uuid,
    pub size: usize,
}

impl GroupId {
    fn new(size: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            size,
        }
    }
}

#[derive(Clone)]
pub enum ServerSender {
    Proxy {
        priority_tx: Sender<ServerEvent>,
        tx: Sender<ServerEvent>,
        wake_up: Arc<dyn Fn() + Send + Sync>,
    },
    Sender {
        priority_tx: Sender<ServerEvent>,
        tx: Sender<ServerEvent>,
    },
    Disconnected,
}

impl ServerSender {
    pub fn send(&self, event: ServerEvent) -> Result<(), SendError<ServerEvent>> {
        let has_priority = event.has_priority();
        self.route(event, has_priority)?;
        self.finish(has_priority);
        Ok(())
    }

    pub fn send_many<E>(&self, events: E) -> Result<(), SendError<ServerEvent>>
    where
        E: IntoIterator<Item = ServerEvent>,
    {
        let mut has_priority = false;
        for event in events {
            let event_has_priority = event.has_priority();
            has_priority |= event_has_priority;
            self.route(event, event_has_priority)?;
        }
        self.finish(has_priority);
        Ok(())
    }

    fn route(&self, event: ServerEvent, has_priority: bool) -> Result<(), SendError<ServerEvent>> {
        match self {
            Self::Proxy {
                tx, priority_tx, ..
            }
            | Self::Sender { priority_tx, tx } => {
                if has_priority {
                    priority_tx.send(event)
                } else {
                    tx.send(event)
                }
            }
            Self::Disconnected => Err(SendError(event)),
        }
    }

    fn finish(&self, has_priority: bool) {
        if has_priority && let Self::Proxy { wake_up, .. } = self {
            wake_up();
        }
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

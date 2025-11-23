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
    world::{BlockHoverData, ChunkData},
};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use uuid::Uuid;
use winit::event_loop::EventLoopProxy;

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
    TimeUpdated(Time),
    ChunkLoaded {
        coords: Point3<i32>,
        data: Arc<ChunkData>,
        group_id: Option<GroupId>,
    },
    ChunkUnloaded {
        coords: Point3<i32>,
        group_id: Option<GroupId>,
    },
    ChunkUpdated {
        coords: Point3<i32>,
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
        tx: Sender<ServerEvent>,
        proxy: EventLoopProxy,
    },
    Sender {
        priority_tx: Sender<ServerEvent>,
        tx: Sender<ServerEvent>,
    },
}

impl ServerSender {
    pub fn disconnected() -> Self {
        let (priority_tx, _) = crossbeam_channel::unbounded();
        let (tx, _) = crossbeam_channel::unbounded();
        Self::Sender { priority_tx, tx }
    }

    pub fn send<E>(&self, events: E) -> Result<(), SendError<ServerEvent>>
    where
        E: IntoIterator<Item = ServerEvent>,
    {
        match self {
            Self::Proxy { tx, proxy } => {
                for event in events {
                    tx.send(event)?;
                }
                proxy.wake_up();
            }
            Self::Sender { priority_tx, tx } => {
                for event in events {
                    if matches!(event, ServerEvent::ClientDisconnected) {
                        priority_tx.send(ServerEvent::ClientDisconnected)?;
                        tx.send(ServerEvent::ClientDisconnected)?;
                    } else if event.has_priority() {
                        priority_tx.send(event)?;
                    } else {
                        tx.send(event)?;
                    }
                }
            }
        }
        Ok(())
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

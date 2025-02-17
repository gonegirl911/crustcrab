pub(crate) mod event_loop;
pub(crate) mod game;
pub(crate) mod ticker;

use self::{
    event_loop::{EventLoop, EventLoopConfig},
    game::{
        Game,
        clock::{ClockState, Time},
        player::PlayerConfig,
        world::{BlockHoverData, ChunkData},
    },
};
use crate::{
    client::{ClientEvent, event_loop::EventLoopProxy},
    shared::utils,
};
use crossbeam_channel::{Receiver, Sender};
use nalgebra::Point3;
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
    Proxy(EventLoopProxy),
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

    pub fn send(&self, event: ServerEvent) -> Result<(), ServerEvent> {
        match self {
            Self::Proxy(proxy) => proxy.send_event(event).map_err(|e| e.0),
            Self::Sender { priority_tx, tx } => {
                if matches!(event, ServerEvent::ClientDisconnected) {
                    _ = priority_tx.send(ServerEvent::ClientDisconnected);
                    _ = tx.send(ServerEvent::ClientDisconnected);
                    Ok(())
                } else if event.has_priority() {
                    priority_tx.send(event).map_err(|e| e.0)
                } else {
                    tx.send(event).map_err(|e| e.0)
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct ServerConfig {
    event_loop: EventLoopConfig,
    player: PlayerConfig,
    clock: ClockState,
}

static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| utils::deserialize("assets/config/server.toml"));

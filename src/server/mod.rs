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
use crossbeam_channel::Receiver;
use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};
use uuid::Uuid;

pub struct Server {
    event_loop: EventLoop,
}

impl Server {
    pub fn new(proxy: EventLoopProxy, client_rx: Receiver<ClientEvent>) -> Self {
        Self {
            event_loop: EventLoop::new(proxy, client_rx),
        }
    }

    pub fn run(self) {
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

#[derive(Deserialize)]
struct ServerConfig {
    event_loop: EventLoopConfig,
    player: PlayerConfig,
    clock: ClockState,
}

static SERVER_CONFIG: LazyLock<ServerConfig> =
    LazyLock::new(|| utils::deserialize("assets/config/server.toml"));

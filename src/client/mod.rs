pub(crate) mod app;
pub(crate) mod event_loop;
pub(crate) mod game;
pub mod net;
pub(crate) mod renderer;
pub(crate) mod stopwatch;
pub(crate) mod window;

use crate::{
    server::{ChunkEvent, ControlEvent, game::world::block::Block},
    shared::toml,
};
use app::{App, AppConfig};
use crossbeam_channel::{Receiver, Sender};
use game::{
    clock::ClockConfig, cloud::CloudConfig, gui::GuiConfig, player::PlayerConfig,
    shading::ShadingConfig, sky::SkyConfig,
};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use winit::event_loop::{ControlFlow, EventLoop};

pub struct Client {
    event_loop: EventLoop,
    player_tx: Sender<PlayerEvent>,
    control_rx: Receiver<ControlEvent>,
    chunk_rx: Receiver<ChunkEvent>,
}

impl Client {
    pub fn new(
        player_tx: Sender<PlayerEvent>,
        control_rx: Receiver<ControlEvent>,
        chunk_rx: Receiver<ChunkEvent>,
    ) -> Self {
        let event_loop = EventLoop::new().expect("event loop should be buildable");
        event_loop.set_control_flow(ControlFlow::Poll);
        Self {
            event_loop,
            player_tx,
            control_rx,
            chunk_rx,
        }
    }

    pub fn run(self) {
        let app = App::new(self.player_tx, self.control_rx, self.chunk_rx);
        self.event_loop
            .run_app(app)
            .expect("event loop should be runnable");
    }
}

#[derive(Serialize, Deserialize)]
pub enum PlayerEvent {
    JoinRequested { render_distance: u32 },
    JoinAcknowledged,
    PositionChanged { origin: Point3<f64> },
    OrientationChanged { dir: Vector3<f32> },
    BlockPlaced(Block),
    BlockDestroyed,
    KeepAlive,
}

#[derive(Deserialize)]
struct ClientConfig {
    player: PlayerConfig,
    sky: SkyConfig,
    cloud: CloudConfig,
    shading: ShadingConfig,
    gui: GuiConfig,
    clock: ClockConfig,
    app: AppConfig,
}

static CLIENT_CONFIG: LazyLock<ClientConfig> =
    LazyLock::new(|| toml::deserialize("assets/config/client.toml"));

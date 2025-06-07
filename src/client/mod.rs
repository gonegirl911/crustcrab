pub(crate) mod app;
pub(crate) mod event_loop;
pub(crate) mod game;
pub(crate) mod renderer;
pub(crate) mod stopwatch;
pub(crate) mod window;

use self::{
    event_loop::{EventLoop, EventLoopProxy},
    game::{cloud::CloudConfig, gui::GuiConfig, player::PlayerConfig, sky::SkyConfig},
};
use crate::{
    server::{ServerSender, game::world::block::Block},
    shared::toml,
};
use app::App;
use crossbeam_channel::Sender;
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use winit::event_loop::ControlFlow;

pub struct Client {
    event_loop: EventLoop,
    client_tx: Sender<ClientEvent>,
}

impl Client {
    pub fn new(client_tx: Sender<ClientEvent>) -> Self {
        env_logger::init();

        let event_loop = EventLoop::with_user_event()
            .build()
            .expect("event loop should be buildable");

        event_loop.set_control_flow(ControlFlow::Poll);

        Self {
            event_loop,
            client_tx,
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        self.event_loop.create_proxy()
    }

    pub fn run(self) {
        let mut app = App::new(self.client_tx);
        self.event_loop
            .run_app(&mut app)
            .expect("event loop should be runnable");
    }
}

#[derive(Serialize, Deserialize)]
pub enum ClientEvent {
    InitialRenderRequested {
        origin: Point3<f32>,
        dir: Vector3<f32>,
        render_distance: u32,
    },
    PlayerPositionChanged {
        origin: Point3<f32>,
    },
    PlayerOrientationChanged {
        dir: Vector3<f32>,
    },
    BlockPlaced(Block),
    BlockDestroyed,
    #[serde(skip)]
    Connected(Box<ServerSender>),
    #[serde(skip)]
    ServerDisconnected,
}

#[derive(Deserialize)]
struct ClientConfig {
    player: PlayerConfig,
    sky: SkyConfig,
    cloud: CloudConfig,
    gui: GuiConfig,
}

static CLIENT_CONFIG: LazyLock<ClientConfig> =
    LazyLock::new(|| toml::deserialize("assets/config/client.toml"));

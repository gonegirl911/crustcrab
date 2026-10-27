#![feature(impl_restriction, mut_restriction)]

mod app;
mod event;
mod frame_budget;
mod game;
pub mod net;
mod renderer;
mod stopwatch;
mod window;

use app::App;
use base::toml;
use crossbeam_channel::{Receiver, Sender};
use domain::protocol::{self, chunk, control};
use game::{clock, cloud, gui, player, shading, sky};
use net::disconnect;
use serde::Deserialize;
use std::sync::LazyLock;
use winit::event_loop::{EventLoop, EventLoopProxy};

pub struct Client {
    event_loop: EventLoop,
}

impl Client {
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            event_loop: EventLoop::new().expect("event loop should be buildable"),
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        self.event_loop.create_proxy()
    }

    pub fn run(
        self,
        player_tx: Sender<protocol::player::Event>,
        control_rx: Receiver<control::Event>,
        chunk_rx: Receiver<chunk::Event>,
        disconnect_rx: Receiver<disconnect::Reason>,
    ) {
        let app = App::new(player_tx, control_rx, chunk_rx, disconnect_rx);
        self.event_loop
            .run_app(app)
            .expect("event loop should be runnable");
    }
}

#[derive(Deserialize)]
struct Config {
    player: player::Config,
    sky: sky::Config,
    cloud: cloud::Config,
    shading: shading::Config,
    gui: gui::Config,
    clock: clock::Config,
}

static CONFIG: LazyLock<Config> =
    LazyLock::new(|| toml::deserialize("client/assets/configs/client.toml"));

pub mod event_loop;
pub mod game;
pub mod renderer;
pub mod stopwatch;
pub mod window;

use self::{
    event_loop::{Event, EventHandler, EventLoop},
    game::{cloud::CloudConfig, gui::GuiConfig, player::PlayerConfig, sky::SkyConfig, Game},
    renderer::Renderer,
    window::Window,
};
use crate::server::{game::world::block::Block, ServerEvent};
use flume::{Receiver, Sender};
use nalgebra::{Point3, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs, time::Duration};
use winit::event_loop::ControlFlow;

pub struct Client {
    event_loop: EventLoop,
    window: Window,
    renderer: Renderer,
    game: Game,
}

impl Client {
    pub async fn new(client_tx: Sender<ClientEvent>, server_rx: Receiver<ServerEvent>) -> Self {
        env_logger::init();

        let event_loop = EventLoop::new(client_tx, server_rx);
        let window = Window::new(&event_loop);
        let renderer = Renderer::new(&window).await;
        let game = Game::new(&renderer);

        Self {
            event_loop,
            window,
            renderer,
            game,
        }
    }

    pub fn run(self) -> ! {
        struct MiniClient {
            window: Window,
            renderer: Renderer,
            game: Game,
        }

        impl EventHandler for MiniClient {
            type Context<'a> = (&'a mut ControlFlow, Sender<ClientEvent>, Duration);

            fn handle(&mut self, event: &Event, (control_flow, client_tx, dt): Self::Context<'_>) {
                self.window.handle(event, control_flow);
                self.renderer.handle(event, ());
                self.game.handle(event, (client_tx, &self.renderer, dt));
            }
        }

        self.event_loop.run(MiniClient {
            window: self.window,
            renderer: self.renderer,
            game: self.game,
        })
    }
}

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
    BlockPlaced {
        block: Block,
    },
    BlockDestroyed,
}

#[derive(Deserialize)]
pub struct ClientConfig {
    player: PlayerConfig,
    sky: SkyConfig,
    cloud: CloudConfig,
    gui: GuiConfig,
}

pub static CLIENT_CONFIG: Lazy<ClientConfig> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/config/client.toml").expect("file should exist"))
        .expect("file should be valid")
});

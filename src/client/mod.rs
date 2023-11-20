pub mod event_loop;
pub mod game;
pub mod renderer;
pub mod window;

use self::{
    event_loop::{Event, EventHandler, EventLoop, EventLoopProxy},
    game::{cloud::CloudConfig, gui::GuiConfig, player::PlayerConfig, sky::SkyConfig, Game},
    renderer::Renderer,
    window::Window,
};
use crate::server::game::world::block::Block;
use flume::Sender;
use nalgebra::{Point3, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::fs;

pub struct Client {
    event_loop: EventLoop,
    window: Window,
    renderer: Renderer,
    game: Game,
}

impl Client {
    pub async fn new(client_tx: Sender<ClientEvent>) -> Self {
        env_logger::init();

        let event_loop = EventLoop::new(client_tx);
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

    pub fn create_proxy(&self) -> EventLoopProxy {
        self.event_loop.create_proxy()
    }

    pub fn run(self) {
        struct MiniClient {
            window: Window,
            renderer: Renderer,
            game: Game,
        }

        impl EventHandler for MiniClient {
            type Context<'a> = &'a Sender<ClientEvent>;

            #[rustfmt::skip]
            fn handle(&mut self, event: &Event, client_tx: Self::Context<'_>) {
                self.window.handle(event, ());
                self.renderer.handle(event, &self.window);
                self.game.handle(event, (client_tx, &self.window, &self.renderer));
            }
        }

        self.event_loop.run(MiniClient {
            window: self.window,
            renderer: self.renderer,
            game: self.game,
        });
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
struct ClientConfig {
    player: PlayerConfig,
    sky: SkyConfig,
    cloud: CloudConfig,
    gui: GuiConfig,
}

static CLIENT_CONFIG: Lazy<ClientConfig> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/config/client.toml").expect("file should exist"))
        .expect("file should be valid")
});

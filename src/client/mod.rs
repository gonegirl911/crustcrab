pub mod event_loop;
pub mod game;
pub mod renderer;
pub mod stopwatch;
pub mod window;

use self::{
    event_loop::{Event, EventHandler, EventLoop},
    game::Game,
    renderer::Renderer,
    window::Window,
};
use crate::server::{game::scene::world::block::Block, ServerEvent};
use flume::{Receiver, Sender};
use nalgebra::{Point3, Vector3};
use std::time::Duration;
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
        player_dir: Vector3<f32>,
        player_coords: Point3<f32>,
        render_distance: u32,
    },
    PlayerOrientationChanged {
        dir: Vector3<f32>,
    },
    PlayerPositionChanged {
        coords: Point3<f32>,
    },
    BlockDestroyed,
    BlockPlaced {
        block: Block,
    },
}

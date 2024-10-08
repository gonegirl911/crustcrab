pub mod event_loop;
pub mod game;
pub mod renderer;
pub mod stopwatch;
pub mod window;

use self::{
    event_loop::{Event, EventHandler, EventLoop, EventLoopProxy},
    game::{Game, cloud::CloudConfig, gui::GuiConfig, player::PlayerConfig, sky::SkyConfig},
    renderer::Renderer,
    window::Window,
};
use crate::{
    client::stopwatch::Stopwatch,
    server::{ServerEvent, game::world::block::Block},
    shared::utils,
};
use crossbeam_channel::Sender;
use nalgebra::{Point3, Vector3};
use serde::Deserialize;
use std::sync::LazyLock;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

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
        struct Instance {
            stopwatch: Stopwatch,
            window: Window,
            renderer: Renderer,
            game: Game,
        }

        impl Instance {
            async fn new(event_loop: &ActiveEventLoop) -> Self {
                let stopwatch = Stopwatch::start();
                let window = Window::new(event_loop);
                let renderer = Renderer::new(&window).await;
                let game = Game::new(&renderer);
                Self {
                    stopwatch,
                    window,
                    renderer,
                    game,
                }
            }
        }

        impl EventHandler for Instance {
            type Context<'a> = &'a Sender<ClientEvent>;

            fn handle(&mut self, event: &Event, client_tx: Self::Context<'_>) {
                self.stopwatch.handle(event, ());
                self.window.handle(event, ());
                self.renderer.handle(event, &self.window);
                self.game.handle(
                    event,
                    (client_tx, &self.window, &self.renderer, self.stopwatch.dt),
                );
            }
        }

        impl ApplicationHandler<ServerEvent> for (Sender<ClientEvent>, Option<Instance>) {
            fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::NewEvents(cause), &self.0);
                }
            }

            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                self.1
                    .get_or_insert_with(|| pollster::block_on(Instance::new(event_loop)))
                    .handle(&Event::Resumed, &self.0);
            }

            fn user_event(&mut self, _: &ActiveEventLoop, event: ServerEvent) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::UserEvent(event), &self.0);
                }
            }

            fn window_event(
                &mut self,
                event_loop: &ActiveEventLoop,
                window_id: WindowId,
                event: WindowEvent,
            ) {
                let should_exit = event == WindowEvent::CloseRequested;

                self.1
                    .as_mut()
                    .unwrap_or_else(|| unreachable!())
                    .handle(&Event::WindowEvent { window_id, event }, &self.0);

                if should_exit {
                    event_loop.exit();
                }
            }

            fn device_event(
                &mut self,
                _: &ActiveEventLoop,
                device_id: DeviceId,
                event: DeviceEvent,
            ) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::DeviceEvent { device_id, event }, &self.0);
                }
            }

            fn about_to_wait(&mut self, _: &ActiveEventLoop) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::AboutToWait, &self.0);
                }
            }

            fn suspended(&mut self, _: &ActiveEventLoop) {
                if let Some(mut instance) = self.1.take() {
                    instance.handle(&Event::Suspended, &self.0);
                }
            }

            fn exiting(&mut self, _: &ActiveEventLoop) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::LoopExiting, &self.0);
                }
            }

            fn memory_warning(&mut self, _: &ActiveEventLoop) {
                if let Some(instance) = &mut self.1 {
                    instance.handle(&Event::MemoryWarning, &self.0);
                }
            }
        }

        self.event_loop
            .run_app(&mut (self.client_tx, None::<Instance>))
            .expect("event loop should be runnable");
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

static CLIENT_CONFIG: LazyLock<ClientConfig> =
    LazyLock::new(|| utils::deserialize("assets/config/client.toml"));

use super::{
    CLIENT_CONFIG, PlayerEvent,
    event_loop::{Event, EventHandler},
    game::Game,
    renderer::{Renderer, Surface},
    stopwatch::Stopwatch,
    window::Window,
};
use crate::server::{ChunkEvent, ControlEvent};
use crossbeam_channel::{Receiver, Sender};
use serde::Deserialize;
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

pub struct App {
    player_tx: Sender<PlayerEvent>,
    control_rx: Receiver<ControlEvent>,
    chunk_rx: Receiver<ChunkEvent>,
    instance: Option<Instance>,
    is_focused: bool,
}

impl App {
    pub fn new(
        player_tx: Sender<PlayerEvent>,
        control_rx: Receiver<ControlEvent>,
        chunk_rx: Receiver<ChunkEvent>,
    ) -> Self {
        Self {
            player_tx,
            control_rx,
            chunk_rx,
            instance: None,
            is_focused: false,
        }
    }

    fn dispatch_server_events(&mut self) {
        let Some(instance) = &mut self.instance else {
            self.control_rx.try_iter().for_each(drop);
            self.chunk_rx.try_iter().for_each(drop);
            return;
        };

        for event in self.control_rx.try_iter() {
            instance.handle(&Event::ControlEvent(event), &self.player_tx);
        }

        let drain_budget = Duration::from_millis(CLIENT_CONFIG.app.drain_budget_ms);
        let deadline = Instant::now() + drain_budget;
        while let Ok(event) = self.chunk_rx.try_recv() {
            instance.handle(&Event::ChunkEvent(event), &self.player_tx);
            if Instant::now() > deadline {
                break;
            }
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _: &dyn ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            assert!(self.instance.is_none());
        } else {
            assert!(self.instance.is_some());
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        assert!(self.instance.is_none());
        self.instance
            .insert(pollster::block_on(Instance::new(event_loop)))
            .handle(&Event::Resumed, &self.player_tx);
    }

    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        unreachable!();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let mut should_exit = false;

        match event {
            WindowEvent::Focused(true) => {
                event_loop.set_control_flow(ControlFlow::Poll);
                self.is_focused = true;
            }
            WindowEvent::Focused(false) => {
                self.is_focused = false;
            }
            WindowEvent::CloseRequested => {
                should_exit = true;
            }
            _ => {}
        }

        self.instance
            .as_mut()
            .unwrap()
            .handle(&Event::WindowEvent(event), &self.player_tx);

        if should_exit {
            event_loop.exit();
        }
    }

    fn device_event(&mut self, _: &dyn ActiveEventLoop, _: Option<DeviceId>, event: DeviceEvent) {
        self.instance
            .as_mut()
            .unwrap()
            .handle(&Event::DeviceEvent(event), &self.player_tx);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.dispatch_server_events();

        if !self.is_focused {
            const WAKE_INTERVAL: Duration = Duration::from_secs(1);

            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + WAKE_INTERVAL));
            return;
        }

        self.instance
            .as_mut()
            .unwrap()
            .handle(&Event::AboutToWait, &self.player_tx);
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        unreachable!();
    }

    fn memory_warning(&mut self, _: &dyn ActiveEventLoop) {
        unreachable!();
    }
}

struct Instance {
    stopwatch: Stopwatch,
    window: Window,
    renderer: Renderer,
    surface: Surface,
    game: Game,
}

impl Instance {
    async fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let stopwatch = Stopwatch::start();
        let window = Window::new(event_loop);
        let (renderer, surface) = Renderer::new(window.0.clone()).await;
        let game = Game::new(&renderer, &surface);
        Self {
            stopwatch,
            window,
            renderer,
            surface,
            game,
        }
    }

    #[rustfmt::skip]
    fn present(&mut self, texture: wgpu::SurfaceTexture) {
        let view = texture.texture.create_view(&Default::default());
        let mut encoder = self.renderer.device.create_command_encoder(&Default::default());
        self.game.draw(&self.renderer, &view, &mut encoder);
        self.renderer.queue.submit([encoder.finish()]);
        self.window.0.pre_present_notify();
        self.renderer.queue.present(texture);
    }

    async fn recover(&mut self) {
        let window = self.window.0.clone();
        if self.renderer.is_device_lost() {
            (self.renderer, self.surface) = Renderer::new(window).await;
            self.game = Game::new(&self.renderer, &self.surface);
        } else {
            self.surface.recreate(window, &self.renderer);
            self.surface.configure(&self.renderer);
        }
    }
}

impl EventHandler for Instance {
    type Context<'a> = &'a Sender<PlayerEvent>;

    #[rustfmt::skip]
    fn handle(&mut self, event: &Event, player_tx: Self::Context<'_>) {
        self.stopwatch.handle(event, ());
        self.window.handle(event, ());
        self.surface.handle(event, (&*self.window.0, &self.renderer));
        self.game.handle(
            event,
            (player_tx, &self.renderer, &self.surface, self.stopwatch.dt),
        );

        if matches!(event, Event::WindowEvent(WindowEvent::RedrawRequested)) {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => {
                    self.present(texture);
                }
                wgpu::CurrentSurfaceTexture::Suboptimal(_)
                | wgpu::CurrentSurfaceTexture::Outdated => {
                    self.surface.configure(&self.renderer);
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {}
                wgpu::CurrentSurfaceTexture::Lost => {
                    pollster::block_on(self.recover());
                }
                wgpu::CurrentSurfaceTexture::Validation => unreachable!(),
            }
        }
    }
}

#[derive(Deserialize)]
pub struct AppConfig {
    pub drain_budget_ms: u64,
}

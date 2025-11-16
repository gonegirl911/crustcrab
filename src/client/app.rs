use super::{
    ClientEvent,
    event_loop::{Event, EventHandler},
    game::Game,
    renderer::Renderer,
    stopwatch::Stopwatch,
    window::Window,
};
use crate::server::ServerEvent;
use crossbeam_channel::{Receiver, Sender};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

pub struct App {
    client_tx: Sender<ClientEvent>,
    server_rx: Receiver<ServerEvent>,
    instance: Option<Instance>,
}

impl App {
    pub fn new(client_tx: Sender<ClientEvent>, server_rx: Receiver<ServerEvent>) -> Self {
        Self {
            client_tx,
            server_rx,
            instance: None,
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
            .handle(&Event::Resumed, &self.client_tx);
    }

    fn proxy_wake_up(&mut self, _: &dyn ActiveEventLoop) {
        for event in self.server_rx.try_iter() {
            self.instance
                .as_mut()
                .unwrap_or_else(|| unreachable!())
                .handle(&Event::ServerEvent(event), &self.client_tx);
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let should_exit = event == WindowEvent::CloseRequested;

        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::WindowEvent(event), &self.client_tx);

        if should_exit {
            event_loop.exit();
        }
    }

    fn device_event(&mut self, _: &dyn ActiveEventLoop, _: Option<DeviceId>, event: DeviceEvent) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::DeviceEvent(event), &self.client_tx);
    }

    fn about_to_wait(&mut self, _: &dyn ActiveEventLoop) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::AboutToWait, &self.client_tx);
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
    game: Game,
}

impl Instance {
    async fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let stopwatch = Stopwatch::start();
        let window = Window::new(event_loop);
        let renderer = Renderer::new(window.clone()).await;
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
        self.renderer.handle(event, &*self.window);
        self.game.handle(
            event,
            (client_tx, &*self.window, &self.renderer, self.stopwatch.dt),
        );
    }
}

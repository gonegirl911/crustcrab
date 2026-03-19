use super::{
    ClientEvent,
    event_loop::{Event, EventHandler},
    game::Game,
    renderer::{Renderer, Surface},
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
        let Some(instance) = &mut self.instance else {
            self.server_rx.try_iter().for_each(drop);
            return;
        };

        for event in self.server_rx.try_iter() {
            instance.handle(&Event::ServerEvent(event), &self.client_tx);
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
    surface: Surface,
    game: Game,
}

impl Instance {
    async fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let stopwatch = Stopwatch::start();
        let window = Window::new(event_loop);
        let (renderer, surface) = Renderer::new(window.to_owned_raw()).await;
        let game = Game::new(&renderer, &surface);
        Self {
            stopwatch,
            window,
            renderer,
            surface,
            game,
        }
    }
}

impl EventHandler for Instance {
    type Context<'a> = &'a Sender<ClientEvent>;

    #[rustfmt::skip]
    fn handle(&mut self, event: &Event, client_tx: Self::Context<'_>) {
        let mut should_recreate_device = false;

        self.stopwatch.handle(event, ());
        self.window.handle(event, ());
        self.surface.handle(event, (self.window.as_raw(), &self.renderer));
        self.game.handle(
            event,
            (
                client_tx,
                self.window.as_raw(),
                &self.renderer,
                &self.surface,
                self.stopwatch.dt,
                &mut should_recreate_device,
            ),
        );

        if should_recreate_device {
            let window = self.window.to_owned_raw();
            if self.renderer.is_device_lost() {
                (self.renderer, self.surface) = pollster::block_on(Renderer::new(window));
                self.game = Game::new(&self.renderer, &self.surface);
            } else {
                self.surface.recreate(window, &self.renderer);
            }
        }
    }
}

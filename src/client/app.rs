use super::{
    ClientEvent,
    event_loop::{Event, EventHandler},
    game::Game,
    renderer::Renderer,
    stopwatch::Stopwatch,
    window::Window,
};
use crate::server::ServerEvent;
use crossbeam_channel::Sender;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

pub struct App {
    client_tx: Sender<ClientEvent>,
    instance: Option<Instance>,
}

impl App {
    pub fn new(client_tx: Sender<ClientEvent>) -> Self {
        Self {
            client_tx,
            instance: None,
        }
    }
}

impl ApplicationHandler<ServerEvent> for App {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if let Some(instance) = &mut self.instance {
            instance.handle(&Event::NewEvents(cause), &self.client_tx);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.instance
            .get_or_insert_with(|| pollster::block_on(Instance::new(event_loop)))
            .handle(&Event::Resumed, &self.client_tx);
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: ServerEvent) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::UserEvent(event), &self.client_tx);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let should_exit = event == WindowEvent::CloseRequested;

        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::WindowEvent { window_id, event }, &self.client_tx);

        if should_exit {
            event_loop.exit();
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::DeviceEvent { device_id, event }, &self.client_tx);
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::AboutToWait, &self.client_tx);
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        unreachable!();
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::LoopExiting, &self.client_tx);
    }

    fn memory_warning(&mut self, _: &ActiveEventLoop) {
        self.instance
            .as_mut()
            .unwrap_or_else(|| unreachable!())
            .handle(&Event::MemoryWarning, &self.client_tx);
    }
}

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
        self.renderer.handle(event, &self.window);
        self.game.handle(
            event,
            (client_tx, &self.window, &self.renderer, self.stopwatch.dt),
        );
    }
}

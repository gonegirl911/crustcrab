use crate::{
    event::{self, Event, Handler as _},
    frame_budget::FrameBudget,
    game::Game,
    net::disconnect,
    renderer::{Renderer, instance::Instance, surface::Surface},
    stopwatch::Stopwatch,
    window::Window,
};
use crossbeam_channel::{Receiver, Sender};
use domain::protocol::{chunk, control, player};
use log::{info, warn};
use std::time::{Duration, Instant};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow},
    window::WindowId,
};

pub(super) struct App {
    player_tx: Sender<player::Event>,
    control_rx: Receiver<control::Event>,
    chunk_rx: Receiver<chunk::Event>,
    disconnect_rx: Receiver<disconnect::Reason>,
    frame_budget: FrameBudget,
    state: Option<State>,
}

impl App {
    const IDLE_WAKEUP_INTERVAL: Duration = Duration::from_millis(50);

    pub(super) fn new(
        player_tx: Sender<player::Event>,
        control_rx: Receiver<control::Event>,
        chunk_rx: Receiver<chunk::Event>,
        disconnect_rx: Receiver<disconnect::Reason>,
    ) -> Self {
        Self {
            player_tx,
            control_rx,
            chunk_rx,
            disconnect_rx,
            frame_budget: Default::default(),
            state: None,
        }
    }

    fn redraw_requested(&mut self) {
        let state = self.state.as_mut().unwrap();

        self.frame_budget
            .begin_frame(Instant::now(), || state.target_frame_interval());

        let scope = self.frame_budget.cpu.time.baseline.measure();
        for event in self.control_rx.try_iter() {
            state.handle(&Event::Control(event), &self.player_tx);
        }
        drop(scope);

        let scope = self.frame_budget.cpu.time.slack.measure();
        for event in self.chunk_rx.try_iter() {
            state.handle(&Event::Chunk(event), &self.player_tx);
            if scope.remaining().is_zero() {
                break;
            }
        }
        drop(scope);

        let scope = self.frame_budget.cpu.time.baseline.measure();
        state.handle(
            &Event::WindowEvent(WindowEvent::RedrawRequested),
            &self.player_tx,
        );
        drop(scope);

        if let Some(texture) = state.get_current_texture(&self.player_tx) {
            {
                let _scope = self.frame_budget.cpu.time.baseline.measure();
                state.draw(&texture.texture);
            }
            state.present(texture);
            self.frame_budget.finish_frame();
        } else {
            self.frame_budget.discard_frame();
        }
    }

    fn idle_wakeup(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        for event in self.control_rx.try_iter() {
            state.handle(&Event::Control(event), &self.player_tx);
        }
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _: &dyn ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            self.idle_wakeup();
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            state.recreate_surface(&self.player_tx);
            info!("surface recreated");
        } else {
            self.state
                .insert(pollster::block_on(State::new(event_loop)))
                .handle(&Event::Init, &self.player_tx);
        }
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        let disconnect_reason = self.disconnect_rx.try_recv().unwrap();
        info!("disconnected: {disconnect_reason}, exiting");
        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let redraw_requested = matches!(event, WindowEvent::RedrawRequested);
        let focused_lost = matches!(event, WindowEvent::Focused(false));
        let close_requested = matches!(event, WindowEvent::CloseRequested);

        if redraw_requested {
            self.redraw_requested();
        } else {
            self.state
                .as_mut()
                .unwrap()
                .handle(&Event::WindowEvent(event), &self.player_tx);
        }

        let state = self.state.as_mut().unwrap();

        if focused_lost {
            state.handle(&Event::InputInvalidated, &self.player_tx);
        }

        if close_requested {
            info!("close requested, exiting");
            event_loop.exit();
        }
    }

    fn device_event(&mut self, _: &dyn ActiveEventLoop, _: Option<DeviceId>, event: DeviceEvent) {
        if let Some(state) = &mut self.state {
            state.handle(&Event::DeviceEvent(event), &self.player_tx);
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Self::IDLE_WAKEUP_INTERVAL,
        ));

        if let Some(state) = &mut self.state {
            state.handle(&Event::AboutToWait, &self.player_tx);

            if state.is_surface_visible() {
                state.request_redraw();
            }
        }
    }

    fn destroy_surfaces(&mut self, _: &dyn ActiveEventLoop) {
        let state = self.state.as_mut().unwrap();
        state.handle(&Event::InputInvalidated, &self.player_tx);
        state.destroy_surface();
        info!("surface destroyed");
    }

    fn memory_warning(&mut self, _: &dyn ActiveEventLoop) {
        warn!("memory warning received");
        if let Some(state) = &mut self.state {
            state.handle(&Event::MemoryWarning, &self.player_tx);
        }
    }
}

struct State {
    stopwatch: Stopwatch,
    window: Window,
    instance: Instance,
    renderer: Renderer,
    surface: Surface,
    game: Game,
}

impl State {
    async fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let stopwatch = Stopwatch::start();
        let window = Window::new(event_loop);
        let instance = Instance::default();
        let (renderer, surface) = Renderer::new(window.raw.clone(), &instance.0).await;
        let game = Game::new(&renderer, &surface);
        Self {
            stopwatch,
            window,
            instance,
            renderer,
            surface,
            game,
        }
    }

    fn recreate_surface(&mut self, player_tx: &Sender<player::Event>) {
        if let Err(e) =
            self.surface
                .recreate(self.window.raw.clone(), &self.instance.0, &self.renderer)
        {
            warn!("recreate surface: {e}, recreating device");
            pollster::block_on(self.recreate_device(player_tx));
        }
    }

    fn is_surface_visible(&self) -> bool {
        self.surface.is_available() && self.window.is_visible()
    }

    fn request_redraw(&self) {
        self.window.raw.request_redraw();
    }

    fn get_current_texture(
        &mut self,
        player_tx: &Sender<player::Event>,
    ) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture()? {
            wgpu::CurrentSurfaceTexture::Success(texture) => Some(texture),
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.request_configure();
                None
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => None,
            wgpu::CurrentSurfaceTexture::Lost => {
                pollster::block_on(self.recover(player_tx));
                None
            }
            wgpu::CurrentSurfaceTexture::Validation => unreachable!(),
        }
    }

    #[rustfmt::skip]
    fn draw(&mut self, texture: &wgpu::Texture) {
        let view = texture.create_view(&Default::default());
        let mut encoder = self.renderer.device.create_command_encoder(&Default::default());
        self.game.draw(&self.renderer, &view, &mut encoder);
        self.renderer.queue.submit([encoder.finish()]);
    }

    fn present(&self, texture: wgpu::SurfaceTexture) {
        self.window.raw.pre_present_notify();
        self.renderer.queue.present(texture);
    }

    async fn recover(&mut self, player_tx: &Sender<player::Event>) {
        if self.renderer.is_device_lost() {
            warn!("device lost, recreating");
            self.recreate_device(player_tx).await;
        } else {
            warn!("surface lost, recreating");
            self.recreate_surface(player_tx);
        }
    }

    async fn recreate_device(&mut self, player_tx: &Sender<player::Event>) {
        (self.renderer, self.surface) =
            Renderer::new(self.window.raw.clone(), &self.instance.0).await;
        self.game = Game::new(&self.renderer, &self.surface);
        self.handle(&Event::Init, player_tx);
    }

    fn destroy_surface(&mut self) {
        self.surface.destroy();
    }

    fn target_frame_interval(&self) -> Option<Duration> {
        let refresh_rate_millihertz = self.window.refresh_rate_millihertz()?;
        Some(Duration::from_secs_f32(
            1000.0 / refresh_rate_millihertz.get() as f32,
        ))
    }
}

impl event::Handler for State {
    type Context<'a> = &'a Sender<player::Event>;

    fn handle(&mut self, event: &Event, player_tx: Self::Context<'_>) {
        self.stopwatch.handle(event, ());
        self.window.handle(event, ());
        self.surface
            .handle(event, (&*self.window.raw, &self.renderer));
        self.game.handle(
            event,
            (player_tx, &self.stopwatch, &self.renderer, &self.surface),
        );
    }
}

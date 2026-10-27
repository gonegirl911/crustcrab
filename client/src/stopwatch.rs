use super::event::{self, Event};
use std::time::{Duration, Instant};
use winit::event::WindowEvent;

pub(super) struct Stopwatch {
    pub(super) mut(self) now: Instant,
    pub(super) mut(self) dt: Duration,
}

impl Stopwatch {
    const MAX_DT: Duration = Duration::from_millis(200);

    pub fn start() -> Self {
        Self {
            now: Instant::now(),
            dt: Default::default(),
        }
    }
}

impl event::Handler for Stopwatch {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if matches!(event, Event::WindowEvent(WindowEvent::RedrawRequested)) {
            let now = Instant::now();
            self.dt = (now - self.now).min(Self::MAX_DT);
            self.now = now;
        }
    }
}

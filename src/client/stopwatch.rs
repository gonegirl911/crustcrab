use super::event_loop::{Event, EventHandler};
use std::time::{Duration, Instant};
use winit::event::WindowEvent;

pub struct Stopwatch {
    pub mut(self) now: Instant,
    pub mut(self) dt: Duration,
}

impl Stopwatch {
    pub fn start() -> Self {
        Self {
            now: Instant::now(),
            dt: Default::default(),
        }
    }
}

impl EventHandler for Stopwatch {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if matches!(event, Event::WindowEvent(WindowEvent::RedrawRequested)) {
            let now = Instant::now();
            self.dt = now - self.now;
            self.now = now;
        }
    }
}

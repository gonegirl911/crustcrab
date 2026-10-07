use crate::client::{
    event_loop::{Event, EventHandler},
    game::player::camera::View,
};
use winit::event::DeviceEvent;

pub struct RotationController {
    dx: f32,
    dy: f32,
    sensitivity: f32,
}

impl RotationController {
    pub fn new(sensitivity: f32) -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            sensitivity,
        }
    }

    pub fn apply(&mut self, view: &mut View) -> bool {
        if self.dx == 0.0 && self.dy == 0.0 {
            return false;
        }

        view.rotate(self.dx * self.sensitivity, self.dy * self.sensitivity);
        self.dx = 0.0;
        self.dy = 0.0;
        true
    }
}

impl EventHandler for RotationController {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let &Event::DeviceEvent(DeviceEvent::PointerMotion { delta: (dx, dy) }) = event {
            self.dx += dx as f32;
            self.dy += dy as f32;
        }
    }
}

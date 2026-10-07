use crate::client::{
    event_loop::{Event, EventHandler},
    game::player::camera::View,
};
use crate::server::ControlEvent;
use bitflags::bitflags;
use nalgebra::Vector3;
use std::time::Duration;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Default)]
pub struct MovementController {
    relevant: Keys,
    history: Keys,
    speed: f64,
}

impl MovementController {
    pub fn apply(&self, view: &mut View, dt: Duration) -> bool {
        if self.relevant.is_empty() {
            return false;
        }

        let mut dir = Vector3::zeros();
        let right = view.right.cast();
        let forward = right.cross(&Vector3::y());

        if self.relevant.contains(Keys::W) {
            dir += forward;
        } else if self.relevant.contains(Keys::S) {
            dir -= forward;
        }

        if self.relevant.contains(Keys::A) {
            dir -= right;
        } else if self.relevant.contains(Keys::D) {
            dir += right;
        }

        if self.relevant.contains(Keys::SPACE) {
            dir.y += 1.0;
        } else if self.relevant.contains(Keys::LSHIFT) {
            dir.y -= 1.0;
        }

        view.origin += dir.normalize() * self.speed * dt.as_secs_f64();
        true
    }

    fn press(&mut self, key: Keys, opp: Keys) {
        self.relevant.insert(key);
        self.relevant.remove(opp);
        self.history.insert(key);
    }

    fn release(&mut self, key: Keys, opp: Keys) {
        self.relevant.remove(key);
        if self.history.contains(opp) {
            self.relevant.insert(opp);
        }
        self.history.remove(key);
    }
}

impl EventHandler for MovementController {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        match event {
            &Event::ControlEvent(ControlEvent::PlayerInitialized { speed, .. }) => {
                self.speed = speed;
            }
            Event::WindowEvent(WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        state,
                        ..
                    },
                ..
            }) => {
                let (key, opp) = match keycode {
                    KeyCode::KeyW => (Keys::W, Keys::S),
                    KeyCode::KeyS => (Keys::S, Keys::W),
                    KeyCode::KeyA => (Keys::A, Keys::D),
                    KeyCode::KeyD => (Keys::D, Keys::A),
                    KeyCode::Space => (Keys::SPACE, Keys::LSHIFT),
                    KeyCode::ShiftLeft => (Keys::LSHIFT, Keys::SPACE),
                    _ => return,
                };

                match state {
                    ElementState::Pressed => {
                        self.press(key, opp);
                    }
                    ElementState::Released => {
                        self.release(key, opp);
                    }
                }
            }
            _ => {}
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Default)]
    struct Keys: u8 {
        const W = 1 << 0;
        const A = 1 << 1;
        const S = 1 << 2;
        const D = 1 << 3;
        const SPACE = 1 << 4;
        const LSHIFT = 1 << 5;
    }
}

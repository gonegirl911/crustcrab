use crate::client::event_loop::{Event, EventHandler};
use bitflags::bitflags;
use nalgebra::{matrix, vector, Matrix4, Point3, Vector3};
use std::{
    f32::consts::{FRAC_PI_2, TAU},
    time::Duration,
};
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, ElementState, KeyboardInput, MouseButton, VirtualKeyCode, WindowEvent},
};

pub struct View {
    pub origin: Point3<f32>,
    pub forward: Vector3<f32>,
    pub right: Vector3<f32>,
    pub up: Vector3<f32>,
    yaw: f32,
    pitch: f32,
}

impl View {
    pub fn new(origin: Point3<f32>, dir: Vector3<f32>) -> Self {
        let forward = dir.normalize();
        let right = Vector3::y().cross(&forward).normalize();
        let up = forward.cross(&right);
        let yaw = forward.z.atan2(forward.x);
        let pitch = forward.y.asin();
        Self {
            origin,
            forward,
            right,
            up,
            yaw,
            pitch,
        }
    }

    pub fn mat(&self) -> Matrix4<f32> {
        matrix![
            self.right.x,   self.right.y,   self.right.z,   -self.origin.coords.dot(&self.right);
            self.up.x,      self.up.y,      self.up.z,      -self.origin.coords.dot(&self.up);
            self.forward.x, self.forward.y, self.forward.z, -self.origin.coords.dot(&self.forward);
            0.0,            0.0,            0.0,            1.0;
        ]
    }
}

pub struct Projection {
    pub fovy: f32,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Projection {
    pub fn new(fovy: f32, aspect: f32, znear: f32, zfar: f32) -> Self {
        Self {
            fovy: fovy.to_radians(),
            aspect,
            znear,
            zfar,
        }
    }

    pub fn mat(&self) -> Matrix4<f32> {
        let h = 1.0 / (self.fovy * 0.5).tan();
        let w = h / self.aspect;
        let r = self.zfar / (self.zfar - self.znear);
        matrix![
            w,   0.0, 0.0, 0.0;
            0.0, h,   0.0, 0.0;
            0.0, 0.0, r,  -r * self.znear;
            0.0, 0.0, 1.0, 0.0;
        ]
    }
}

#[derive(Default)]
pub struct Controller {
    dx: f32,
    dy: f32,
    relevant_keys: Keys,
    key_history: Keys,
    aspect: f32,
    relevant_buttons: MouseButtons,
    button_history: MouseButtons,
    speed: f32,
    sensitivity: f32,
}

impl Controller {
    pub fn new(aspect: f32, speed: f32, sensitivity: f32) -> Self {
        Self {
            aspect,
            speed,
            sensitivity,
            ..Default::default()
        }
    }

    pub fn apply_updates(
        &mut self,
        view: &mut View,
        projection: &mut Projection,
        dt: Duration,
    ) -> Changes {
        let mut changes = Changes::empty();

        if self.dx != 0.0 || self.dy != 0.0 {
            self.apply_rotation(view);
            self.dx = 0.0;
            self.dy = 0.0;
            changes.insert(Changes::ROTATED);
        }

        if !self.relevant_keys.is_empty() {
            self.apply_movement(view, dt);
            changes.insert(Changes::MOVED);
        }

        if self.aspect != 0.0 {
            projection.aspect = self.aspect;
            self.aspect = 0.0;
            changes.insert(Changes::RESIZED);
        }

        if self.relevant_buttons.contains(MouseButtons::LEFT) {
            changes.insert(Changes::BLOCK_DESTROYED);
        } else if self.relevant_buttons.contains(MouseButtons::RIGHT) {
            changes.insert(Changes::BLOCK_PLACED);
        }

        changes
    }

    fn apply_rotation(&mut self, view: &mut View) {
        const BOUND_Y: f32 = FRAC_PI_2 - f32::EPSILON;

        view.yaw = (view.yaw - self.dx * self.sensitivity) % TAU;
        view.pitch = (view.pitch - self.dy * self.sensitivity).clamp(-BOUND_Y, BOUND_Y);
        view.forward = Self::forward(view.yaw, view.pitch);
        view.right = Vector3::y().cross(&view.forward).normalize();
        view.up = view.forward.cross(&view.right);
    }

    fn apply_movement(&mut self, view: &mut View, dt: Duration) {
        let mut dir = Vector3::zeros();
        let right = view.right;
        let up = Vector3::y();
        let forward = right.cross(&up);

        if self.relevant_keys.contains(Keys::W) {
            dir += forward;
        } else if self.relevant_keys.contains(Keys::S) {
            dir -= forward;
        }

        if self.relevant_keys.contains(Keys::A) {
            dir -= right;
        } else if self.relevant_keys.contains(Keys::D) {
            dir += right;
        }

        if self.relevant_keys.contains(Keys::SPACE) {
            dir += up;
        } else if self.relevant_keys.contains(Keys::LSHIFT) {
            dir -= up;
        }

        view.origin += dir.normalize() * self.speed * dt.as_secs_f32();
    }

    fn forward(yaw: f32, pitch: f32) -> Vector3<f32> {
        vector![
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos()
        ]
    }
}

impl EventHandler for Controller {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        match event {
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: (dx, dy) },
                ..
            } => {
                self.dx += *dx as f32;
                self.dy += *dy as f32;
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            state,
                            ..
                        },
                    ..
                } => {
                    let (key, opp) = match keycode {
                        VirtualKeyCode::W => (Keys::W, Keys::S),
                        VirtualKeyCode::A => (Keys::A, Keys::D),
                        VirtualKeyCode::S => (Keys::S, Keys::W),
                        VirtualKeyCode::D => (Keys::D, Keys::A),
                        VirtualKeyCode::Space => (Keys::SPACE, Keys::LSHIFT),
                        VirtualKeyCode::LShift => (Keys::LSHIFT, Keys::SPACE),
                        _ => return,
                    };
                    match state {
                        ElementState::Pressed => {
                            self.relevant_keys.insert(key);
                            self.relevant_keys.remove(opp);
                            self.key_history.insert(key);
                        }
                        ElementState::Released => {
                            self.relevant_keys.remove(key);
                            if self.key_history.contains(opp) {
                                self.relevant_keys.insert(opp);
                            }
                            self.key_history.remove(key);
                        }
                    }
                }
                WindowEvent::Resized(PhysicalSize { width, height })
                | WindowEvent::ScaleFactorChanged {
                    new_inner_size: PhysicalSize { width, height },
                    ..
                } if *width != 0 && !height != 0 => {
                    self.aspect = *width as f32 / *height as f32;
                }
                WindowEvent::MouseInput { button, state, .. } => {
                    let (button, opp) = match button {
                        MouseButton::Left => (MouseButtons::LEFT, MouseButtons::RIGHT),
                        MouseButton::Right => (MouseButtons::RIGHT, MouseButtons::LEFT),
                        _ => return,
                    };
                    match state {
                        ElementState::Pressed => {
                            self.relevant_buttons.insert(button);
                            self.relevant_buttons.remove(opp);
                            self.button_history.insert(button);
                        }
                        ElementState::Released => {
                            self.relevant_buttons.remove(button);
                            if self.button_history.contains(opp) {
                                self.relevant_buttons.insert(opp);
                            }
                            self.button_history.remove(button);
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

bitflags! {
    pub struct Changes: u8 {
        const MOVED = 1 << 0;
        const ROTATED = 1 << 1;
        const RESIZED = 1 << 2;
        const BLOCK_PLACED = 1 << 3;
        const BLOCK_DESTROYED = 1 << 4;
        const MATRIX_CHANGES = Self::MOVED.bits() | Self::ROTATED.bits() | Self::RESIZED.bits();
    }

    #[derive(Clone, Copy, Default)]
    struct Keys: u8 {
        const W = 1 << 0;
        const A = 1 << 1;
        const S = 1 << 2;
        const D = 1 << 3;
        const SPACE = 1 << 4;
        const LSHIFT = 1 << 5;
    }

    #[derive(Clone, Copy, Default)]
    struct MouseButtons: u8 {
        const LEFT = 1 << 0;
        const RIGHT = 1 << 1;
    }
}

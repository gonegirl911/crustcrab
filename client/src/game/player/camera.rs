use base::position::Position3;
use nalgebra::{Matrix4, Vector3, matrix, vector};
use std::f32::consts::{FRAC_PI_2, TAU};

pub(super) struct View {
    pub(super) origin: Position3,
    pub(super) mut(self) forward: Vector3<f32>,
    pub(super) mut(self) right: Vector3<f32>,
    pub(super) mut(self) up: Vector3<f32>,
    yaw: f32,
    pitch: f32,
}

impl View {
    pub(super) fn new(origin: Position3, dir: Vector3<f32>) -> Self {
        let forward = dir.normalize();
        let right = Self::right(forward);
        let up = Self::up(forward, right);
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

    pub(super) fn mat(&self) -> Matrix4<f32> {
        matrix![
            self.right.x,   self.right.y,   self.right.z,   0.0;
            self.up.x,      self.up.y,      self.up.z,      0.0;
            self.forward.x, self.forward.y, self.forward.z, 0.0;
            0.0,            0.0,            0.0,            1.0;
        ]
    }

    pub(super) fn rotate(&mut self, dx: f32, dy: f32) {
        const BOUND_Y: f32 = FRAC_PI_2 - f32::EPSILON;

        self.yaw = (self.yaw - dx) % TAU;
        self.pitch = (self.pitch - dy).clamp(-BOUND_Y, BOUND_Y);
        self.forward = Self::forward(self.yaw, self.pitch);
        self.right = Self::right(self.forward);
        self.up = Self::up(self.forward, self.right);
    }

    fn forward(yaw: f32, pitch: f32) -> Vector3<f32> {
        vector![
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos()
        ]
    }

    fn right(forward: Vector3<f32>) -> Vector3<f32> {
        Vector3::y().cross(&forward).normalize()
    }

    fn up(forward: Vector3<f32>, right: Vector3<f32>) -> Vector3<f32> {
        forward.cross(&right)
    }
}

pub(super) struct Projection {
    pub(super) mut(self) fovy: f32,
    pub(super) aspect: f32,
    pub(super) mut(self) znear: f32,
    pub(super) mut(self) zfar: f32,
}

impl Projection {
    pub(super) fn new(fovy: f32, aspect: f32, znear: f32, zfar: f32) -> Self {
        Self {
            fovy: fovy.to_radians(),
            aspect,
            znear,
            zfar,
        }
    }

    pub(super) fn mat(&self) -> Matrix4<f32> {
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

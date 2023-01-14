use crate::client::event_loop::{Event, EventHandler};
use nalgebra::{matrix, Matrix4};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Projection {
    fovy: f32,
    aspect: f32,
    znear: f32,
    zfar: f32,
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

    pub fn fovy(&self) -> f32 {
        self.fovy
    }

    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    pub fn znear(&self) -> f32 {
        self.znear
    }

    pub fn zfar(&self) -> f32 {
        self.zfar
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

impl EventHandler for Projection {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: PhysicalSize { width, height },
                        ..
                    },
                ..
            } if *width != 0 && *height != 0 => {
                self.aspect = *width as f32 / *height as f32;
            }
            _ => {}
        }
    }
}

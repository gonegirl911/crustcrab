mod crosshair;

use crate::{
    CONFIG,
    event::{self, Event},
    renderer::{Renderer, surface::Surface},
};
use crosshair::Crosshair;
use nalgebra::{Matrix4, Vector2, vector};
use serde::Deserialize;

pub(super) struct Gui {
    crosshair: Crosshair,
}

impl Gui {
    pub(super) fn new(renderer: &Renderer) -> Self {
        Self {
            crosshair: Crosshair::new(renderer),
        }
    }

    pub(super) fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.crosshair.draw(&mut render_pass);
    }

    fn unit(surface: &Surface) -> Vector2<f32> {
        let surface_width = surface.config.width as f32;
        let surface_height = surface.config.height as f32;
        let size = (surface_height * CONFIG.gui.base_unit).max(CONFIG.gui.min_size);
        vector![size / surface_width, size / surface_height]
    }

    fn transform(scaling: Vector2<f32>, offset: Vector2<f32>) -> Matrix4<f32> {
        Matrix4::new_translation(&vector![-1.0, -1.0, 0.0])
            .prepend_nonuniform_scaling(&vector![2.0, 2.0, 1.0])
            .prepend_translation(&vector![offset.x, offset.y, 0.0])
            .prepend_nonuniform_scaling(&vector![scaling.x, scaling.y, 1.0])
    }
}

impl event::Handler for Gui {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.crosshair.handle(event, (renderer, surface));
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    base_unit: f32,
    min_size: f32,
    crosshair: crosshair::Config,
}

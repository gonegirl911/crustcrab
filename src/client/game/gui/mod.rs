pub mod crosshair;
pub mod inventory;

use crate::client::{
    CLIENT_CONFIG,
    event_loop::{Event, EventHandler},
    renderer::{
        Renderer, Surface,
        effect::{Blit, Effect, PostProcessor},
    },
};
use crosshair::{Crosshair, CrosshairConfig};
use inventory::{Inventory, InventoryConfig};
use nalgebra::{Matrix4, Vector2, vector};
use serde::Deserialize;

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
    pub mut(self) inventory: Inventory,
}

impl Gui {
    pub fn new(
        renderer: &Renderer,
        surface: &Surface,
        lighting_bind_group_layout: &wgpu::BindGroupLayout,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            blit: Blit::new(renderer, input_bind_group_layout, PostProcessor::FORMAT),
            crosshair: Crosshair::new(renderer, surface, input_bind_group_layout),
            inventory: Inventory::new(
                renderer,
                lighting_bind_group_layout,
                textures_bind_group_layout,
            ),
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        lighting_bind_group: &wgpu::BindGroup,
        input_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
    ) {
        {
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
            self.blit.draw(&mut render_pass, input_bind_group);
            self.crosshair.draw(&mut render_pass, input_bind_group);
        }
        self.inventory.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            }),
            lighting_bind_group,
            textures_bind_group,
        );
    }

    fn scaling(surface: &Surface, factor: f32) -> Vector2<f32> {
        let config = &CLIENT_CONFIG.gui;
        let size = (surface.height() * config.base_unit).max(config.min_size) * factor;
        vector![size / surface.width(), size / surface.height()]
    }

    fn transform(scaling: Vector2<f32>, offset: Vector2<f32>) -> Matrix4<f32> {
        Matrix4::new_translation(&vector![-1.0, -1.0, 0.0])
            .prepend_nonuniform_scaling(&vector![2.0, 2.0, 1.0])
            .prepend_translation(&vector![offset.x, offset.y, 0.0])
            .prepend_nonuniform_scaling(&vector![scaling.x, scaling.y, 1.0])
    }
}

impl EventHandler for Gui {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.crosshair.handle(event, (renderer, surface));
        self.inventory.handle(event, (renderer, surface));
    }
}

#[derive(Deserialize)]
pub struct GuiConfig {
    base_unit: f32,
    min_size: f32,
    crosshair: CrosshairConfig,
    inventory: InventoryConfig,
}

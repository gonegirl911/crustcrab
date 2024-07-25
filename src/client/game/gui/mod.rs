pub mod crosshair;
pub mod inventory;

use self::{
    crosshair::{Crosshair, CrosshairConfig},
    inventory::{Inventory, InventoryConfig},
};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            effect::{Blit, Effect, PostProcessor},
            Renderer,
        },
    },
    server::game::world::block::Block,
};
use nalgebra::{vector, Matrix4, Vector2};
use serde::Deserialize;

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
    inventory: Inventory,
}

impl Gui {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            blit: Blit::new(renderer, input_bind_group_layout, PostProcessor::FORMAT),
            crosshair: Crosshair::new(renderer, input_bind_group_layout),
            inventory: Inventory::new(renderer, textures_bind_group_layout),
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.inventory.selected_block()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        input_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
    ) {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            textures_bind_group,
        );
    }

    fn scaling(Renderer { config, .. }: &Renderer, factor: f32) -> Vector2<f32> {
        let size = (config.height as f32 * 0.0325).max(13.5) * factor;
        vector![size / config.width as f32, size / config.height as f32]
    }

    fn transform(scaling: Vector2<f32>, offset: Vector2<f32>) -> Matrix4<f32> {
        Matrix4::new_translation(&vector![-1.0, -1.0, 0.0])
            .prepend_nonuniform_scaling(&vector![2.0, 2.0, 1.0])
            .prepend_translation(&vector![offset.x, offset.y, 0.0])
            .prepend_nonuniform_scaling(&vector![scaling.x, scaling.y, 1.0])
    }
}

impl EventHandler for Gui {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.crosshair.handle(event, renderer);
        self.inventory.handle(event, renderer);
    }
}

#[derive(Deserialize)]
pub struct GuiConfig {
    crosshair: CrosshairConfig,
    inventory: InventoryConfig,
}

pub mod crosshair;
pub mod inventory;

use self::{crosshair::Crosshair, inventory::Inventory};
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            effect::{Blit, Effect, PostProcessor},
            Renderer,
        },
    },
    server::game::world::{block::Block, chunk::Chunk},
};
use arrayvec::ArrayVec;
use nalgebra::{vector, Matrix4, Point3};
use serde::Deserialize;
use std::fs;

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
    inventory: Inventory,
    state: ClientState,
}

impl Gui {
    pub fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let blit = Blit::new(renderer, input_bind_group_layout, PostProcessor::FORMAT);
        let crosshair = Crosshair::new(renderer, input_bind_group_layout);
        let state = ClientState::new();
        let inventory = Inventory::new(
            renderer,
            state.inventory.clone(),
            textures_bind_group_layout,
        );
        Self {
            blit,
            crosshair,
            inventory,
            state,
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        input_bind_group: &wgpu::BindGroup,
        textures_bind_group: &wgpu::BindGroup,
    ) {
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });
            self.blit.draw(&mut render_pass, input_bind_group);
            self.crosshair.draw(&mut render_pass, input_bind_group);
        }
        {
            self.inventory.draw(
                &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: true,
                        }),
                        stencil_ops: None,
                    }),
                }),
                textures_bind_group,
            );
        }
    }

    pub fn selected_block(&self) -> Option<Block> {
        self.inventory.selected_block()
    }

    pub fn render_distance(&self) -> u32 {
        self.state.render_distance
    }

    pub fn origin(&self) -> Point3<f32> {
        self.state.origin
    }

    pub fn fovy(&self) -> f32 {
        self.state.fovy
    }

    pub fn zfar(&self) -> f32 {
        ((self.render_distance() + 1) * Chunk::DIM as u32) as f32
    }

    pub fn speed(&self) -> f32 {
        self.state.speed
    }

    pub fn sensitivity(&self) -> f32 {
        self.state.sensitivity
    }

    fn element_scaling(size: f32) -> Matrix4<f32> {
        Matrix4::new_nonuniform_scaling(&vector![size, size, 1.0])
    }

    fn element_size(Renderer { config, .. }: &Renderer, factor: f32) -> f32 {
        (config.height as f32 * 0.0325).max(13.5) * factor
    }

    fn viewport(Renderer { config, .. }: &Renderer) -> Matrix4<f32> {
        Matrix4::new_translation(&vector![-1.0, -1.0, 0.0])
            * Matrix4::new_nonuniform_scaling(&vector![
                2.0 / config.width as f32,
                2.0 / config.height as f32,
                1.0
            ])
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
struct ClientState {
    inventory: ArrayVec<Block, 9>,
    render_distance: u32,
    origin: Point3<f32>,
    fovy: f32,
    speed: f32,
    sensitivity: f32,
}

impl ClientState {
    fn new() -> Self {
        toml::from_str(&fs::read_to_string("assets/client.toml").expect("file should exist"))
            .expect("file should be valid")
    }
}

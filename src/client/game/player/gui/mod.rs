mod crosshair;

use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::Renderer,
    },
    server::game::scene::world::block::Block,
};
use crosshair::Crosshair;

pub struct Gui {
    crosshair: Crosshair,
}

impl Gui {
    pub fn new(renderer: &Renderer, output_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            crosshair: Crosshair::new(renderer, output_bind_group_layout),
        }
    }

    pub fn selected_block(&self) -> Block {
        Block::Grass
    }

    pub fn render_distance(&self) -> u32 {
        36
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        output_bind_group: &wgpu::BindGroup,
    ) {
        self.crosshair.draw(
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
                depth_stencil_attachment: None,
            }),
            output_bind_group,
        );
    }
}

impl EventHandler for Gui {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.crosshair.handle(event, renderer);
    }
}

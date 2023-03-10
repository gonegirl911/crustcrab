pub mod crosshair;

use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{Blit, Effect, Renderer},
    },
    server::game::world::block::Block,
};
use crosshair::Crosshair;

pub struct Gui {
    blit: Blit,
    crosshair: Crosshair,
}

impl Gui {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            blit: Blit::new(renderer, input_bind_group_layout, None),
            crosshair: Crosshair::new(renderer, input_bind_group_layout),
        }
    }

    pub fn selected_block(&self) -> Block {
        Block::Glowstone
    }

    pub fn render_distance(&self) -> u32 {
        36
    }
}

impl Effect for Gui {
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
        self.blit.draw(render_pass, input_bind_group);
        self.crosshair.draw(render_pass, input_bind_group);
    }
}

impl EventHandler for Gui {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.crosshair.handle(event, renderer);
    }
}

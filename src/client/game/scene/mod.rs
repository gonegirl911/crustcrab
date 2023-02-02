pub mod clock;
pub mod depth_buffer;
pub mod selection;
pub mod sky;
pub mod world;

use self::{
    clock::Clock, depth_buffer::DepthBuffer, selection::BlockSelection, sky::Sky, world::World,
};
use super::player::frustum::Frustum;
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};

pub struct Scene {
    clock: Clock,
    sky: Sky,
    world: World,
    block_selection: BlockSelection,
    depth_buffer: DepthBuffer,
}

impl Scene {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let clock = Clock::new(renderer);
        let sky = Sky::new(
            renderer,
            player_bind_group_layout,
            clock.bind_group_layout(),
        );
        let world = World::new(
            renderer,
            player_bind_group_layout,
            clock.bind_group_layout(),
            sky.bind_group_layout(),
        );
        let block_selection = BlockSelection::new(renderer, player_bind_group_layout);
        let depth_buffer = DepthBuffer::new(renderer);
        Self {
            clock,
            sky,
            world,
            block_selection,
            depth_buffer,
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        frustum: &Frustum,
    ) {
        let render_pass = &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: self.depth_buffer.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        #[rustfmt::skip]
        self.sky.draw(render_pass, player_bind_group, self.clock.bind_group());
        self.world.draw(
            render_pass,
            player_bind_group,
            self.clock.bind_group(),
            self.sky.bind_group(),
            frustum,
        );
        self.block_selection.draw(render_pass, player_bind_group);
    }
}

impl EventHandler for Scene {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.clock.handle(event, renderer);
        self.world.handle(event, renderer);
        self.block_selection.handle(event, ());
        self.depth_buffer.handle(event, renderer);
    }
}

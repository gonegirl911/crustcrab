pub mod clock;
pub mod depth;
pub mod hover;
pub mod light;
pub mod sky;
pub mod world;

use self::{
    clock::Clock, depth::DepthBuffer, hover::BlockHover, light::Skylight, sky::Sky, world::World,
};
use super::player::frustum::Frustum;
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
};

pub struct Scene {
    clock: Clock,
    skylight: Skylight,
    sky: Sky,
    world: World,
    block_hover: BlockHover,
    depth_buffer: DepthBuffer,
}

impl Scene {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let clock = Clock::new(renderer);
        let skylight = Skylight::new(renderer);
        let sky = Sky::new(
            renderer,
            player_bind_group_layout,
            clock.bind_group_layout(),
        );
        let world = World::new(
            renderer,
            player_bind_group_layout,
            clock.bind_group_layout(),
            skylight.bind_group_layout(),
            sky.bind_group_layout(),
        );
        let block_hover = BlockHover::new(
            renderer,
            player_bind_group_layout,
            skylight.bind_group_layout(),
        );
        let depth_buffer = DepthBuffer::new(renderer);
        Self {
            clock,
            skylight,
            sky,
            world,
            block_hover,
            depth_buffer,
        }
    }

    #[rustfmt::skip]
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
        self.sky.draw(render_pass, player_bind_group, self.clock.bind_group());
        self.world.draw(
            render_pass,
            player_bind_group,
            self.clock.bind_group(),
            self.skylight.bind_group(),
            self.sky.bind_group(),
            frustum,
        );
        self.block_hover.draw(render_pass, player_bind_group, self.skylight.bind_group());
    }
}

impl EventHandler for Scene {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.clock.handle(event, renderer);
        self.skylight.handle(event, renderer);
        self.world.handle(event, renderer);
        self.block_hover.handle(event, ());
        self.depth_buffer.handle(event, renderer);
    }
}

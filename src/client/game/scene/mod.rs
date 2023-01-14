pub mod clock;
pub mod depth_buffer;
pub mod player;
pub mod sky;
pub mod world;

use self::{clock::Clock, depth_buffer::DepthBuffer, player::Player, sky::Sky, world::World};
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::Renderer,
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Scene {
    player: Player,
    clock: Clock,
    sky: Sky,
    depth_buffer: DepthBuffer,
    world: World,
}

impl Scene {
    pub fn new(renderer: &Renderer) -> Self {
        let player = Player::new(renderer);
        let clock = Clock::new(renderer);
        let sky = Sky::new(
            renderer,
            player.bind_group_layout(),
            clock.bind_group_layout(),
        );
        let depth_buffer = DepthBuffer::new(renderer);
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            clock.bind_group_layout(),
            sky.bind_group_layout(),
        );
        Self {
            player,
            clock,
            sky,
            depth_buffer,
            world,
        }
    }

    pub fn render(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.sky.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            self.player.bind_group(),
            self.clock.bind_group(),
        );
        self.world.draw(
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
                    view: self.depth_buffer.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            }),
            self.player.bind_group(),
            self.clock.bind_group(),
            self.sky.bind_group(),
            &self.player.frustum(),
        );
    }
}

impl EventHandler for Scene {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, dt): Self::Context<'_>) {
        self.player.handle(event, (client_tx, renderer, dt));
        self.clock.handle(event, renderer);
        self.depth_buffer.handle(event, renderer);
        self.world.handle(event, renderer);
    }
}

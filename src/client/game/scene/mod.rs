pub mod clock;
pub mod depth_buffer;
pub mod player;
pub mod selection;
pub mod sky;
pub mod world;

use self::{
    clock::Clock, depth_buffer::DepthBuffer, player::Player, selection::BlockSelection, sky::Sky,
    world::World,
};
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{Bindable, Renderer, Viewable},
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
    block_selection: BlockSelection,
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
        let block_selection = BlockSelection::new(renderer, player.bind_group_layout());
        Self {
            player,
            clock,
            sky,
            depth_buffer,
            world,
            block_selection,
        }
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        self.sky.draw(
            &mut render_pass,
            self.player.bind_group(),
            self.clock.bind_group(),
        );

        self.world.draw(
            &mut render_pass,
            self.player.bind_group(),
            self.clock.bind_group(),
            self.sky.bind_group(),
            &self.player.frustum(),
        );

        self.block_selection
            .draw(&mut render_pass, self.player.bind_group());
    }
}

impl EventHandler for Scene {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    fn handle(&mut self, event: &Event, (client_tx, renderer, dt): Self::Context<'_>) {
        self.player.handle(event, (client_tx, renderer, dt));
        self.clock.handle(event, renderer);
        self.depth_buffer.handle(event, renderer);
        self.world.handle(event, renderer);
        self.block_selection.handle(event, ());
    }
}

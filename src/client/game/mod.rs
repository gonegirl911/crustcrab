pub mod gui;
pub mod hover;
pub mod player;
pub mod sky;
pub mod world;

use self::{gui::Gui, hover::BlockHover, player::Player, sky::Sky, world::World};
use super::{
    event_loop::{Event, EventHandler},
    renderer::{DepthBuffer, PostProcessor, Renderer},
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Game {
    gui: Gui,
    player: Player,
    sky: Sky,
    world: World,
    hover: BlockHover,
    depth_buffer: DepthBuffer,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let processor = PostProcessor::new(renderer);
        let gui = Gui::new(renderer, processor.bind_group_layout());
        let player = Player::new(renderer, &gui);
        let sky = Sky::new(renderer);
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
        );
        let hover = BlockHover::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
        );
        let depth_buffer = DepthBuffer::new(renderer);
        Self {
            gui,
            player,
            sky,
            world,
            hover,
            depth_buffer,
            processor,
        }
    }

    fn draw(&mut self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.sky.draw(self.processor.view(), encoder);
        self.world.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth_buffer.view(),
            &self.player.frustum(),
        );
        self.hover.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth_buffer.view(),
        );
        self.processor.blit_apply(encoder, &self.gui);
        self.processor.draw(view, encoder);
    }
}

impl EventHandler for Game {
    type Context<'a> = (Sender<ClientEvent>, &'a Renderer, Duration);

    #[rustfmt::skip]
    fn handle(
        &mut self,
        event: &Event,
        (
            client_tx,
            renderer @ Renderer {
                surface,
                device,
                queue,
                ..
            },
            dt,
        ): Self::Context<'_>,
    ) {
        self.gui.handle(event, renderer);
        self.player.handle(event, (client_tx, renderer, &self.gui, dt));
        self.world.handle(event, renderer);
        self.hover.handle(event, ());
        self.depth_buffer.handle(event, renderer);
        self.processor.handle(event, renderer);

        if let Event::RedrawRequested(_) = event {
            match surface.get_current_texture() {
                Ok(surface) => {
                    let view = surface.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.draw(&view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                Err(_) => {},
            }
        }
    }
}

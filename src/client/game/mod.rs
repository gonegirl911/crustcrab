pub mod gui;
pub mod hover;
pub mod player;
pub mod sky;
pub mod world;

use self::{
    gui::Gui,
    hover::BlockHover,
    player::Player,
    sky::Sky,
    world::{BlockTextureArray, World},
};
use super::{
    event_loop::{Event, EventHandler},
    renderer::{
        effect::{Aces, PostProcessor},
        texture::screen::DepthBuffer,
        Renderer,
    },
    ClientEvent,
};
use flume::Sender;
use std::time::Duration;

pub struct Game {
    textures: BlockTextureArray,
    gui: Gui,
    player: Player,
    sky: Sky,
    world: World,
    hover: BlockHover,
    aces: Aces,
    depth: DepthBuffer,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let textures = BlockTextureArray::new(renderer);
        let processor = PostProcessor::new(renderer);
        let gui = Gui::new(
            renderer,
            processor.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let player = Player::new(renderer, &gui);
        let sky = Sky::new(renderer);
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let hover = BlockHover::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
        );
        let aces = Aces::new(
            renderer,
            processor.bind_group_layout(),
            PostProcessor::FORMAT,
        );
        let depth = DepthBuffer::new(renderer);
        Self {
            textures,
            gui,
            player,
            sky,
            world,
            aces,
            hover,
            depth,
            processor,
        }
    }

    fn draw(&mut self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        self.world.draw(
            self.processor.view(),
            encoder,
            self.depth.view(),
            self.player.bind_group(),
            self.sky.bind_group(),
            self.textures.bind_group(),
            &self.player.frustum(),
        );
        self.hover.draw(
            self.processor.view(),
            encoder,
            self.depth.view(),
            self.player.bind_group(),
            self.sky.bind_group(),
        );
        self.processor.apply(encoder, &self.aces);
        self.processor.apply_raw(|view, bind_group| {
            self.gui.draw(
                view,
                encoder,
                self.depth.view(),
                bind_group,
                self.textures.bind_group(),
            );
        });
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
        } else {
            self.gui.handle(event, renderer);
            self.player.handle(event, (client_tx, renderer, &self.gui, dt));
            self.sky.handle(event, renderer);
            self.world.handle(event, renderer);
            self.hover.handle(event, ());
            self.depth.handle(event, renderer);
            self.processor.handle(event, renderer);
        }
    }
}

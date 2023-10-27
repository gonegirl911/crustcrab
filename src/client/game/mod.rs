pub mod cloud;
pub mod fog;
pub mod gui;
pub mod hover;
pub mod player;
pub mod sky;
pub mod world;

use self::{
    cloud::CloudLayer, fog::Fog, gui::Gui, hover::BlockHover, player::Player, sky::Sky,
    world::World,
};
use super::{
    event_loop::{Event, EventHandler},
    renderer::{
        effect::{Aces, PostProcessor},
        texture::{image::ImageTextureArray, screen::DepthBuffer},
        Renderer,
    },
    ClientEvent,
};
use crate::server::game::world::block::data::TEX_PATHS;
use flume::Sender;
use std::{ops::Deref, time::Duration};

pub struct Game {
    gui: Gui,
    player: Player,
    sky: Sky,
    world: World,
    clouds: CloudLayer,
    fog: Fog,
    hover: BlockHover,
    aces: Aces,
    textures: BlockTextureArray,
    depth: DepthBuffer,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let textures = BlockTextureArray::new(renderer);
        let depth = DepthBuffer::new(renderer);
        let processor = PostProcessor::new(renderer);
        let gui = Gui::new(
            renderer,
            processor.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let player = Player::new(renderer);
        let sky = Sky::new(renderer, player.bind_group_layout());
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let clouds = CloudLayer::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            processor.bind_group_layout(),
        );
        let fog = Fog::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            depth.bind_group_layout(),
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
        Self {
            gui,
            player,
            sky,
            world,
            clouds,
            fog,
            hover,
            aces,
            textures,
            depth,
            processor,
        }
    }

    #[rustfmt::skip]
    fn draw(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.sky.draw(self.processor.view(), encoder, self.player.bind_group());
        self.world.draw(
            renderer,
            self.fog.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.textures.bind_group(),
            self.depth.view(),
            &self.player.frustum(),
            |encoder| {
                self.fog.draw(
                    self.processor.view(),
                    encoder,
                    self.player.bind_group(),
                    self.sky.bind_group(),
                    self.depth.bind_group(),
                );
            },
        );
        self.fog.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth.bind_group(),
        );
        self.clouds.draw(
            self.fog.view(),
            encoder,
            self.processor.spare_view(),
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth.view(),
            self.processor.spare_bind_group(),
        );
        self.fog.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth.bind_group(),
        );
        self.hover.draw(
            self.processor.view(),
            encoder,
            self.player.bind_group(),
            self.sky.bind_group(),
            self.depth.view(),
        );
        self.processor.apply(encoder, &self.aces);
        self.processor.apply_raw(|view, bind_group| {
            self.gui.draw(
                view,
                encoder,
                bind_group,
                self.textures.bind_group(),
                self.depth.view(),
            );
        });
        self.processor.draw(view, encoder);
    }
}

impl EventHandler for Game {
    type Context<'a> = (&'a Sender<ClientEvent>, &'a Renderer, Duration);

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
                    self.draw(renderer, &view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    surface.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                Err(_) => {}
            }
        } else {
            self.gui.handle(event, renderer);
            self.player.handle(event, (client_tx, renderer, &self.gui.inventory, dt));
            self.sky.handle(event, renderer);
            self.world.handle(event, renderer);
            self.clouds.handle(event, dt);
            self.fog.handle(event, renderer);
            self.hover.handle(event, ());
            self.depth.handle(event, renderer);
            self.processor.handle(event, renderer);
        }
    }
}

struct BlockTextureArray(ImageTextureArray);

impl BlockTextureArray {
    fn new(renderer: &Renderer) -> Self {
        Self(ImageTextureArray::new(
            renderer,
            Self::tex_paths(),
            true,
            true,
            4,
        ))
    }

    fn tex_paths() -> impl Iterator<Item = String> {
        TEX_PATHS
            .iter()
            .map(|path| format!("assets/textures/blocks/{path}"))
    }
}

impl Deref for BlockTextureArray {
    type Target = ImageTextureArray;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

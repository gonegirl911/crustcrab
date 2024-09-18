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
    window::RawWindow,
    ClientEvent,
};
use crate::server::game::world::block::data::TEX_PATHS;
use crossbeam_channel::Sender;
use std::{ops::Deref, time::Duration};
use winit::event::WindowEvent;

pub struct Game {
    sky: Sky,
    world: World,
    clouds: CloudLayer,
    fog: Fog,
    hover: BlockHover,
    aces: Aces,
    gui: Gui,
    player: Player,
    textures: BlockTextureArray,
    depth: DepthBuffer,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer) -> Self {
        let player = Player::new(renderer);
        let sky = Sky::new(renderer, player.bind_group_layout());
        let textures = BlockTextureArray::new(renderer);
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let processor = PostProcessor::new(renderer);
        let clouds = CloudLayer::new(
            renderer,
            player.bind_group_layout(),
            processor.bind_group_layout(),
        );
        let depth = DepthBuffer::new(renderer);
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
        let gui = Gui::new(
            renderer,
            processor.bind_group_layout(),
            textures.bind_group_layout(),
        );
        Self {
            sky,
            world,
            clouds,
            fog,
            hover,
            aces,
            gui,
            player,
            textures,
            depth,
            processor,
        }
    }

    fn draw(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.sky
            .draw(self.processor.view(), encoder, self.player.bind_group());

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

                self.hover.draw(
                    self.processor.view(),
                    encoder,
                    self.player.bind_group(),
                    self.sky.bind_group(),
                    self.depth.view(),
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
    type Context<'a> = (
        &'a Sender<ClientEvent>,
        &'a RawWindow,
        &'a Renderer,
        Duration,
    );

    #[rustfmt::skip]
    fn handle(
        &mut self,
        event: &Event,
        (
            client_tx,
            window,
            renderer @ Renderer {
                surface,
                device,
                queue,
                ..
            },
            dt,
        ): Self::Context<'_>,
    ) {
        self.sky.handle(event, renderer);
        self.world.handle(event, renderer);
        self.clouds.handle(event, dt);
        self.fog.handle(event, renderer);
        self.hover.handle(event, ());
        self.gui.handle(event, renderer);
        self.player.handle(event, (client_tx, renderer, &self.gui, dt));
        self.depth.handle(event, renderer);
        self.processor.handle(event, renderer);

        if let Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } = event
        {
            match surface.get_current_texture() {
                Ok(texture) => {
                    let view = texture.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.draw(renderer, &view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    window.pre_present_notify();
                    texture.present();
                }
                Err(wgpu::SurfaceError::Lost) => renderer.recreate_surface(),
                Err(_) => {}
            }
        }
    }
}

struct BlockTextureArray(ImageTextureArray);

impl BlockTextureArray {
    fn new(renderer: &Renderer) -> Self {
        Self(ImageTextureArray::new(
            renderer,
            Self::tex_paths(),
            4,
            true,
            wgpu::AddressMode::Repeat,
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

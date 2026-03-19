pub mod cloud;
pub mod fog;
pub mod gui;
pub mod hover;
pub mod player;
pub mod sky;
pub mod world;

use super::{
    ClientEvent,
    event_loop::{Event, EventHandler},
    renderer::{
        Renderer, Surface,
        effect::{Aces, PostProcessor},
        texture::{image::ImageTextureArray, screen::DepthBuffer},
    },
    window::RawWindow,
};
use crate::{client::renderer::utils::load_rgba, server::game::world::block::data::TEX_PATHS};
use cloud::CloudLayer;
use crossbeam_channel::Sender;
use fog::Fog;
use gui::Gui;
use hover::BlockHover;
use image::RgbaImage;
use player::Player;
use sky::Sky;
use std::{ops::Deref, time::Duration};
use winit::event::WindowEvent;
use world::World;

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
    pub fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let player = Player::new(renderer);
        let sky = Sky::new(renderer, surface, player.bind_group_layout());
        let textures = BlockTextureArray::new(renderer, surface);
        let world = World::new(
            renderer,
            player.bind_group_layout(),
            sky.bind_group_layout(),
            textures.bind_group_layout(),
        );
        let processor = PostProcessor::new(renderer, surface);
        let clouds = CloudLayer::new(
            renderer,
            surface,
            player.bind_group_layout(),
            processor.bind_group_layout(),
        );
        let depth = DepthBuffer::new(renderer, surface);
        let fog = Fog::new(
            renderer,
            surface,
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
            surface,
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
        &'a Surface,
        Duration,
        &'a mut bool,
    );

    #[rustfmt::skip]
    fn handle(
        &mut self,
        event: &Event,
        (
            client_tx,
            window,
            renderer @ Renderer { device, queue, .. },
            surface,
            dt,
            is_surface_texture_lost,
        ): Self::Context<'_>,
    ) {
        self.sky.handle(event, renderer);
        self.world.handle(event, renderer);
        self.clouds.handle(event, dt);
        self.fog.handle(event, (renderer, surface));
        self.hover.handle(event, ());
        self.gui.handle(event, (renderer, surface));
        self.player.handle(event, (client_tx, renderer, surface, &self.gui, dt));
        self.depth.handle(event, (renderer, surface));
        self.processor.handle(event, (renderer, surface));

        if matches!(event, Event::WindowEvent(WindowEvent::RedrawRequested)) {
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(texture) => {
                    let view = texture.texture.create_view(&Default::default());
                    let mut encoder = device.create_command_encoder(&Default::default());
                    self.draw(renderer, &view, &mut encoder);
                    queue.submit([encoder.finish()]);
                    window.pre_present_notify();
                    texture.present();
                }
                wgpu::CurrentSurfaceTexture::Suboptimal(_)
                | wgpu::CurrentSurfaceTexture::Outdated => {
                    surface.reconfigure(renderer);
                }
                wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {}
                wgpu::CurrentSurfaceTexture::Lost => {
                    *is_surface_texture_lost = true;
                }
                wgpu::CurrentSurfaceTexture::Validation => unreachable!(),
            }
        }
    }
}

struct BlockTextureArray(ImageTextureArray);

impl BlockTextureArray {
    fn new(renderer: &Renderer, surface: &Surface) -> Self {
        Self(
            ImageTextureArray::builder()
                .renderer(renderer)
                .surface(surface)
                .images(Self::images())
                .mip_level_count(4)
                .is_srgb(true)
                .address_mode(wgpu::AddressMode::Repeat)
                .build(),
        )
    }

    fn images() -> impl Iterator<Item = RgbaImage> {
        TEX_PATHS
            .iter()
            .map(|path| load_rgba(format!("assets/textures/blocks/{path}")))
    }
}

impl Deref for BlockTextureArray {
    type Target = ImageTextureArray;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

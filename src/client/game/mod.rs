pub mod clock;
pub mod cloud;
pub mod fog;
pub mod gui;
pub mod highlight;
pub mod player;
pub mod shading;
pub mod sky;
pub mod world;

use super::{
    PlayerEvent,
    event_loop::{Event, EventHandler},
    renderer::{
        Renderer, Surface,
        effect::{Aces, PostProcessor},
        texture::{image::ImageTextureArray, screen::DepthBuffer},
        utils::color_pass,
    },
};
use crate::{
    client::renderer::utils::load_rgba,
    server::{ControlEvent, game::world::block::data::TEX_PATHS},
};
use clock::Clock;
use cloud::CloudLayer;
use crossbeam_channel::Sender;
use fog::Fog;
use gui::Gui;
use highlight::BlockHighlight;
use player::Player;
use shading::Shading;
use sky::Sky;
use std::{ops::Deref, time::Duration};
use world::World;

pub struct Game {
    clock: Clock,
    sky: Sky,
    shading: Shading,
    world: World,
    clouds: CloudLayer,
    fog: Fog,
    highlight: BlockHighlight,
    aces: Aces,
    gui: Gui,
    player: Player,
    textures: BlockTextureArray,
    depth: DepthBuffer,
    processor: PostProcessor,
}

impl Game {
    pub fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let clock = Clock::default();
        let player = Player::new(renderer);
        let sky = Sky::new(renderer, surface, &player.uniform.bind_group_layout);
        let shading = Shading::new(renderer);
        let textures = BlockTextureArray::new(renderer, surface);
        let world = World::new(
            renderer,
            &player.uniform.bind_group_layout,
            &shading.uniform.bind_group_layout,
            &shading.texture.bind_group_layout,
            &textures.bind_group_layout,
        );
        let processor = PostProcessor::new(renderer, surface);
        let clouds = CloudLayer::new(
            renderer,
            surface,
            &player.uniform.bind_group_layout,
            &shading.uniform.bind_group_layout,
            processor.bind_group_layout(),
        );
        let depth = DepthBuffer::new(renderer, surface);
        let fog = Fog::new(
            renderer,
            surface,
            &player.uniform.bind_group_layout,
            &sky.uniform.bind_group_layout,
            depth.bind_group_layout(),
        );
        let highlight = BlockHighlight::new(
            renderer,
            &player.uniform.bind_group_layout,
            &shading.texture.bind_group_layout,
        );
        let aces = Aces::new(
            renderer,
            processor.bind_group_layout(),
            PostProcessor::FORMAT,
        );
        let gui = Gui::new(
            renderer,
            surface,
            &shading.uniform.bind_group_layout,
            processor.bind_group_layout(),
            &textures.bind_group_layout,
        );
        Self {
            clock,
            sky,
            shading,
            world,
            clouds,
            fog,
            highlight,
            aces,
            gui,
            player,
            textures,
            depth,
            processor,
        }
    }

    pub fn draw(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let time = self.clock.time();
        let nightness = time.nightness();
        let origin = self.player.view.origin;
        let anchor = self.player.view.anchor();
        let frustum = self.player.frustum();

        self.sky.draw(
            renderer,
            self.processor.view(),
            encoder,
            &self.player.uniform.bind_group,
            time,
        );

        self.shading.update(renderer, nightness);

        let blended_points = self.world.draw_opaque(
            self.fog.texture.view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.shading.uniform.bind_group,
            &self.shading.texture.bind_group,
            &self.textures.bind_group,
            self.depth.view(),
            anchor,
            &frustum,
        );

        self.fog.draw(
            self.processor.view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth.bind_group(),
        );

        self.highlight.draw(
            self.processor.view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.shading.texture.bind_group,
            self.depth.view(),
            anchor,
        );

        self.world.draw_blended(
            renderer,
            self.fog.texture.view(),
            encoder,
            blended_points,
            &self.player.uniform.bind_group,
            &self.shading.uniform.bind_group,
            &self.shading.texture.bind_group,
            &self.textures.bind_group,
            self.depth.view(),
            origin,
            anchor,
        );

        self.fog.draw(
            self.processor.view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth.bind_group(),
        );

        self.clouds.draw(
            self.fog.texture.view(),
            encoder,
            self.processor.spare_view(),
            &self.player.uniform.bind_group,
            &self.shading.uniform.bind_group,
            self.depth.view(),
            self.processor.spare_bind_group(),
            origin,
            time,
        );

        self.fog.draw(
            self.processor.view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth.bind_group(),
        );

        self.processor.step(|view, bind_group| {
            self.aces.draw(
                &mut color_pass(view, encoder, wgpu::LoadOp::Clear(Default::default())),
                bind_group,
            );
        });

        self.processor.step(|view, bind_group| {
            self.gui.draw(
                view,
                encoder,
                &self.shading.uniform.bind_group,
                bind_group,
                &self.textures.bind_group,
                self.depth.view(),
            );
        });

        self.processor.draw(&mut color_pass(
            view,
            encoder,
            wgpu::LoadOp::Clear(Default::default()),
        ));
    }
}

impl EventHandler for Game {
    type Context<'a> = (&'a Sender<PlayerEvent>, &'a Renderer, &'a Surface, Duration);

    #[rustfmt::skip]
    fn handle(&mut self, event: &Event, (player_tx, renderer, surface, dt): Self::Context<'_>) {
        self.clock.handle(event, dt);
        self.world.handle(event, renderer);
        self.fog.handle(event, (renderer, surface));
        self.highlight.handle(event, ());
        self.gui.handle(event, (renderer, surface));
        self.player.handle(event, (player_tx, renderer, surface, &self.gui, dt));
        self.depth.handle(event, (renderer, surface));
        self.processor.handle(event, (renderer, surface));

        if let Event::ControlEvent(ControlEvent::JoinFinished) = event {
            _ = player_tx.send(PlayerEvent::JoinAcknowledged);
        }
    }
}

struct BlockTextureArray(ImageTextureArray);

impl BlockTextureArray {
    fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let images = TEX_PATHS
            .iter()
            .map(|path| load_rgba(format!("assets/textures/blocks/{path}")))
            .collect::<Vec<_>>();
        Self(
            ImageTextureArray::builder()
                .renderer(renderer)
                .surface(surface)
                .images(&images)
                .mip_level_count(4)
                .is_srgb(true)
                .address_mode(wgpu::AddressMode::Repeat)
                .build(),
        )
    }
}

impl Deref for BlockTextureArray {
    type Target = ImageTextureArray;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

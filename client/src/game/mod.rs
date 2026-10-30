mod block_highlight;
pub(super) mod clock;
pub(super) mod cloud;
mod fog;
pub(super) mod gui;
mod output;
pub(super) mod player;
pub(super) mod shading;
pub(super) mod sky;
mod world;

use crate::{
    event::{self, Event},
    renderer::{
        Renderer, post_process::PostProcessor, surface::Surface,
        texture::screen::depth::DepthTexture,
    },
    stopwatch::Stopwatch,
};
use block_highlight::BlockHighlight;
use clock::Clock;
use crossbeam_channel::Sender;
use domain::protocol::{self, control};
use fog::Fog;
use gui::Gui;
use output::Output;
use player::Player;
use shading::Shading;
use sky::Sky;

pub(super) struct Game {
    post_processor: PostProcessor,
    output: Output,
    clock: Clock,
    player: Player,
    sky: Sky,
    clouds: cloud::Layer,
    fog: Fog,
    highlight: BlockHighlight,
    gui: Gui,
    depth_texture: DepthTexture,
    shading: Shading,
}

impl Game {
    pub(super) fn new(renderer: &Renderer, surface: &Surface) -> Self {
        let post_processor = PostProcessor::new(renderer, surface);
        let output = Output::new(renderer, surface, post_processor.bind_group_layout());
        let depth_texture = DepthTexture::new(renderer, surface);
        let shading = Shading::new(renderer);
        let clock = Default::default();
        let player = Player::new(renderer);
        let sky = Sky::new(renderer, &player.uniform.bind_group_layout);
        let clouds = cloud::Layer::new(
            renderer,
            &player.uniform.bind_group_layout,
            &shading.uniform.bind_group_layout,
            post_processor.bind_group_layout(),
        );
        let fog = Fog::new(
            renderer,
            surface,
            &player.uniform.bind_group_layout,
            &sky.uniform.bind_group_layout,
            depth_texture.bind_group_layout(),
        );
        let highlight = BlockHighlight::new(
            renderer,
            &player.uniform.bind_group_layout,
            &shading.texture.bind_group_layout,
        );
        let gui = Gui::new(renderer);
        Self {
            post_processor,
            output,
            clock,
            player,
            sky,
            clouds,
            fog,
            highlight,
            gui,
            depth_texture,
            shading,
        }
    }

    #[rustfmt::skip]
    pub(super) fn draw(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let time = self.clock.render_time();
        let camera = self.player.render_camera();

        self.shading.update(renderer, &time);

        self.sky.draw(
            renderer,
            self.post_processor.current_view(),
            encoder,
            &self.player.uniform.bind_group,
            &time,
        );

        todo!();

        self.fog.draw(
            self.post_processor.current_view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth_texture.bind_group(),
        );

        self.highlight.draw(
            self.post_processor.current_view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.shading.texture.bind_group,
            self.depth_texture.view(),
            &camera,
        );

        todo!();

        self.fog.draw(
            self.post_processor.current_view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth_texture.bind_group(),
        );

        self.clouds.draw(
            self.fog.texture.view(),
            encoder,
            self.post_processor.spare_view(),
            &self.player.uniform.bind_group,
            &self.shading.uniform.bind_group,
            self.depth_texture.view(),
            self.post_processor.spare_bind_group(),
            &time,
            &camera,
        );

        self.fog.draw(
            self.post_processor.current_view(),
            encoder,
            &self.player.uniform.bind_group,
            &self.sky.uniform.bind_group,
            self.depth_texture.bind_group(),
        );

        self.post_processor.tonemap(encoder);
        self.post_processor.encode_srgb(encoder);

        self.gui.draw(self.post_processor.current_view(), encoder);

        self.post_processor.dither(encoder);
        self.post_processor.decode_srgb(encoder);

        self.output.draw(view, encoder, self.post_processor.current_bind_group());
    }
}

impl event::Handler for Game {
    type Context<'a> = (
        &'a Sender<protocol::player::Event>,
        &'a Stopwatch,
        &'a Renderer,
        &'a Surface,
    );

    #[rustfmt::skip]
    fn handle(
        &mut self,
        event: &Event,
        (player_tx, stopwatch, renderer, surface): Self::Context<'_>,
    ) {
        self.post_processor.handle(event, (renderer, surface));
        self.output.handle(event, (renderer, surface));
        self.clock.handle(event, stopwatch);
        self.player.handle(event, (player_tx, stopwatch, renderer, surface));
        self.fog.handle(event, (renderer, surface));
        self.highlight.handle(event, ());
        self.gui.handle(event, (renderer, surface));
        self.depth_texture.handle(event, (renderer, surface));

        if let Event::Control(control::Event::JoinFinished) = event {
            _ = player_tx.send(protocol::player::Event::JoinAcknowledged);
        }
    }
}

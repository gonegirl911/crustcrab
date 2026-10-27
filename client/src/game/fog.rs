use crate::{
    event::{self, Event},
    renderer::{
        Renderer,
        post_process::PostProcessor,
        render_pass::color_pass,
        render_pipeline::{RenderPipeline, read_wgsl},
        surface::Surface,
        texture::screen::ScreenTexture,
    },
};

pub(super) struct Fog {
    pub(super) mut(self) texture: ScreenTexture,
    render_pipeline: RenderPipeline,
}

impl Fog {
    pub(super) fn new(
        renderer: &Renderer,
        surface: &Surface,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        sky_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        depth_texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = ScreenTexture::new(renderer, surface, PostProcessor::FORMAT);
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("client/assets/shaders/fog.wgsl"))
            .bind_group_layouts(&[
                Some(player_uniform_bind_group_layout),
                Some(sky_uniform_bind_group_layout),
                Some(texture.bind_group_layout()),
                Some(depth_texture_bind_group_layout),
            ])
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING)
            .build();
        Self {
            texture,
            render_pipeline,
        }
    }

    pub(super) fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_uniform_bind_group: &wgpu::BindGroup,
        sky_uniform_bind_group: &wgpu::BindGroup,
        depth_texture_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = color_pass(view, encoder, wgpu::LoadOp::Load);
        self.render_pipeline.bind(
            &mut render_pass,
            [
                player_uniform_bind_group,
                sky_uniform_bind_group,
                self.texture.bind_group(),
                depth_texture_bind_group,
            ],
        );
        render_pass.draw(0..3, 0..1);
    }
}

impl event::Handler for Fog {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, event: &Event, (renderer, surface): Self::Context<'_>) {
        self.texture.handle(event, (renderer, surface));
    }
}

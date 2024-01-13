use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{effect::PostProcessor, program::Program, texture::screen::ScreenTexture, Renderer},
};

pub struct Fog {
    texture: ScreenTexture,
    program: Program,
}

impl Fog {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
        depth_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let texture = ScreenTexture::new(renderer, PostProcessor::FORMAT);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/fog.wgsl"),
            &[],
            &[
                player_bind_group_layout,
                sky_bind_group_layout,
                texture.bind_group_layout(),
                depth_bind_group_layout,
            ],
            &[],
            None,
            None,
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        Self { texture, program }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        depth_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        self.program.bind(
            &mut render_pass,
            [
                player_bind_group,
                sky_bind_group,
                self.texture.bind_group(),
                depth_bind_group,
            ],
        );
        render_pass.draw(0..3, 0..1);
    }
}

impl EventHandler for Fog {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}

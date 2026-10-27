use crate::renderer::{
    self, Immediates as _, Renderer,
    render_pipeline::{RenderPipeline, read_wgsl},
};
use bytemuck::{Pod, Zeroable};

pub(crate) struct Blender(RenderPipeline);

impl Blender {
    pub(crate) fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("client/assets/shaders/blender.wgsl"))
                .bind_group_layouts(&[Some(input_bind_group_layout)])
                .immediate_size(Immediates::SIZE)
                .format(format)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
        )
    }

    pub(crate) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        input_bind_group: &wgpu::BindGroup,
        opacity: f32,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        Immediates::new(opacity).set(render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Immediates {
    opacity: f32,
}

impl Immediates {
    fn new(opacity: f32) -> Self {
        Self { opacity }
    }
}

impl renderer::Immediates for Immediates {}

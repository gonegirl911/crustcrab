use crate::renderer::{
    Renderer,
    render_pipeline::{RenderPipeline, read_wgsl},
};

pub(in crate::renderer) struct Dither(RenderPipeline);

impl Dither {
    pub(in crate::renderer) fn new(
        renderer: &Renderer,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        Self(
            RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("client/assets/shaders/dither.wgsl"))
                .bind_group_layouts(&[Some(input_bind_group_layout)])
                .format(format)
                .build(),
        )
    }

    pub(in crate::renderer) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        input_bind_group: &wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [input_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

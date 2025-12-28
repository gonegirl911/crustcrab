use crate::client::renderer::{
    Renderer, effect::PostProcessor, program::Program, utils::read_wgsl,
};

pub struct Atmosphere(Program);

impl Atmosphere {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self(
            Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/atmosphere.wgsl"))
                .bind_group_layouts(&[player_bind_group_layout, sky_bind_group_layout])
                .format(PostProcessor::FORMAT)
                .build(),
        )
    }

    #[rustfmt::skip]
    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [player_bind_group, sky_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

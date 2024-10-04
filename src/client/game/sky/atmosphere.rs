use crate::client::renderer::{Renderer, effect::PostProcessor, program::Program};

pub struct Atmosphere(Program);

impl Atmosphere {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[player_bind_group_layout, sky_bind_group_layout],
            &[],
            None,
            None,
            PostProcessor::FORMAT,
            None,
        ))
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

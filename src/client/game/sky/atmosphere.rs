use crate::client::renderer::{effect::PostProcessor, program::Program, Renderer};

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
    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [player_bind_group, sky_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

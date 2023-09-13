use crate::client::renderer::{effect::PostProcessor, program::Program, Renderer};

pub struct Atmosphere(Program);

impl Atmosphere {
    pub fn new(renderer: &Renderer, sky_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/atmosphere.wgsl"),
            &[],
            &[sky_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        ))
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        sky_bind_group: &'a wgpu::BindGroup,
    ) {
        self.0.bind(render_pass, [sky_bind_group]);
        render_pass.draw(0..3, 0..1);
    }
}

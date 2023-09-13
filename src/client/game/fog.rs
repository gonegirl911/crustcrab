use crate::client::renderer::{effect::PostProcessor, program::Program, Renderer};

pub struct Fog(Program);

impl Fog {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
        input_bind_group_layout: &wgpu::BindGroupLayout,
        depth_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self(Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/fog.wgsl"),
            &[],
            &[
                player_bind_group_layout,
                sky_bind_group_layout,
                input_bind_group_layout,
                depth_bind_group_layout,
            ],
            &[],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        ))
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        input_bind_group: &wgpu::BindGroup,
        depth_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.0.bind(
            &mut render_pass,
            [
                player_bind_group,
                sky_bind_group,
                input_bind_group,
                depth_bind_group,
            ],
        );
        render_pass.draw(0..3, 0..1);
    }
}

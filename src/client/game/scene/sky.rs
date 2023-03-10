use crate::{
    client::renderer::{Renderer, Uniform},
    color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};

pub struct Sky {
    uniform: Uniform<SkyUniformData>,
}

impl Sky {
    const COLOR: Rgb<f32> = Rgb::splat(0.0);
    const LIGHT_INTENSITY: Rgb<f32> = Rgb::new(0.15, 0.15, 0.3);

    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::with_data(
                renderer,
                &SkyUniformData::new(Self::COLOR, Self::LIGHT_INTENSITY),
                wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Self::COLOR.into()),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    color: Float3,
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(color: Rgb<f32>, light_intensity: Rgb<f32>) -> Self {
        Self {
            color: color.into(),
            light_intensity: light_intensity.into(),
        }
    }
}

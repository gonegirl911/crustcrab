use crate::{
    client::renderer::{Renderer, Uniform},
    color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};

pub struct Sky(Uniform<SkyUniformData>);

impl Sky {
    pub fn new(renderer: &Renderer) -> Self {
        Self(Uniform::with_constant_data(
            renderer,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            &SkyUniformData::new(Rgb::new(0.15, 0.15, 0.3)),
        ))
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.0.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.0.bind_group()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(light_intensity: Rgb<f32>) -> Self {
        Self {
            light_intensity: light_intensity.into(),
        }
    }
}

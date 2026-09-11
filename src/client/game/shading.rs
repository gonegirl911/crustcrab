use crate::{
    client::{
        CLIENT_CONFIG,
        renderer::{Renderer, buffer::MemoryState, uniform::Uniform},
    },
    server::game::world::block::data::SideShade,
    shared::enum_map::EnumMap,
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;

pub struct Shading {
    uniform: Uniform<ShadingUniformData>,
}

impl Shading {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(
                renderer,
                MemoryState::Immutable(&Default::default()),
                wgpu::ShaderStages::VERTEX,
            ),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ShadingUniformData {
    side_factors: [f32; 4],
    ao_factor_min: f32,
    ao_factor_max: f32,
    light_attenuation: f32,
}

impl Default for ShadingUniformData {
    fn default() -> Self {
        let config = &CLIENT_CONFIG.shading;
        Self {
            side_factors: config.side_factors.inner().into_array(),
            ao_factor_min: config.ao_factor_min,
            ao_factor_max: config.ao_factor_max,
            light_attenuation: config.light_attenuation,
        }
    }
}

#[derive(Deserialize)]
pub struct ShadingConfig {
    pub side_factors: EnumMap<SideShade, f32>,
    pub ao_factor_min: f32,
    pub ao_factor_max: f32,
    pub light_attenuation: f32,
}

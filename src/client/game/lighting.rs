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

pub struct Lighting {
    uniform: Uniform<LightingUniformData>,
}

impl Lighting {
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
struct LightingUniformData {
    side_factors: [f32; 4],
    attenuation: f32,
    ao_factor_min: f32,
    ao_factor_max: f32,
    padding: f32,
}

impl Default for LightingUniformData {
    fn default() -> Self {
        let config = &CLIENT_CONFIG.lighting;
        Self {
            side_factors: config.side_factors.inner().into_array(),
            attenuation: config.attenuation,
            ao_factor_min: config.ao_factor_min,
            ao_factor_max: config.ao_factor_max,
            padding: Default::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct LightingConfig {
    pub side_factors: EnumMap<SideShade, f32>,
    pub attenuation: f32,
    pub ao_factor_min: f32,
    pub ao_factor_max: f32,
}

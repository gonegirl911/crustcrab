use super::{clock::RenderTime, world::mesh::SideShade};
use crate::{
    CONFIG,
    renderer::{
        Renderer,
        buffer::MemoryState,
        texture::data::{DataTexture, Rgba16Unorm},
        uniform::Uniform,
    },
};
use base::{
    color::{Rgb, Rgba},
    math::interpolation::lerp,
    math::quantize,
};
use bytemuck::{Pod, Zeroable};
use domain::game::world::block;
use enum_map::EnumMap;
use serde::Deserialize;
use std::array;

pub(super) struct Shading {
    pub(super) mut(self) uniform: Uniform<UniformData>,
    pub(super) mut(self) texture: DataTexture<Rgba16Unorm>,
    uploaded_nightness: Option<f32>,
}

impl Shading {
    pub(super) fn new(renderer: &Renderer) -> Self {
        Self {
            uniform: Uniform::new(
                renderer,
                MemoryState::Immutable(&Default::default()),
                wgpu::ShaderStages::VERTEX,
            ),
            texture: DataTexture::builder()
                .renderer(renderer)
                .size(wgpu::Extent3d {
                    width: LightTable::AO_LEVELS as u32,
                    height: LightTable::LIGHT_LEVELS as u32,
                    depth_or_array_layers: LightTable::LIGHT_LEVELS as u32,
                })
                .dimension(wgpu::TextureDimension::D3)
                .visibility(wgpu::ShaderStages::VERTEX)
                .build(),
            uploaded_nightness: None,
        }
    }

    #[rustfmt::skip]
    pub(super) fn update(&mut self, renderer: &Renderer, &RenderTime { nightness, .. }: &RenderTime) {
        if self.uploaded_nightness.replace(nightness) != Some(nightness) {
            let table = LightTable::new(nightness);
            self.texture.write(renderer, bytemuck::cast_slice(table.as_slice()));
        }
    }
}

struct LightTable([[[Rgba<u16>; Self::AO_LEVELS]; Self::LIGHT_LEVELS]; Self::LIGHT_LEVELS]);

impl LightTable {
    const AO_LEVELS: usize = 4;
    const LIGHT_LEVELS: usize = block::Light::COMPONENT_MAX as usize + 1;

    #[rustfmt::skip]
    fn new(nightness: f32) -> Self {
        let sunlight_intensity = CONFIG.sky.sunlight_intensity(nightness);
        Self(array::from_fn(|torchlight| {
            array::from_fn(|skylight| {
                array::from_fn(|ao| {
                    let world_light = Self::world_light(
                        skylight as u8,
                        torchlight as u8,
                        sunlight_intensity,
                    );
                    let ao_factor = Self::ao_factor(ao as u8);
                    let value = world_light * ao_factor;
                    value.with_alpha(1.0).map(quantize::to_unorm16)
                })
            })
        }))
    }

    fn value(&self, light: block::Light, ao: u8) -> Rgb<f32> {
        let skylight = light.skylight();
        let torchlight = light.torchlight();
        Rgb::from_fn(|i| {
            let quantized_value = self.0[torchlight[i] as usize][skylight[i] as usize][ao as usize];
            quantize::from_unorm16(quantized_value[i])
        })
    }

    fn as_slice(&self) -> &[Rgba<u16>] {
        self.0.as_flattened().as_flattened()
    }

    fn world_light(skylight: u8, torchlight: u8, sunlight_intensity: Rgb<f32>) -> Rgb<f32> {
        let light_attenuation = CONFIG.shading.light_attenuation;
        let light_max = block::Light::COMPONENT_MAX;

        let global_light = light_attenuation.powi((light_max - skylight) as i32);
        let local_light = light_attenuation.powi((light_max - torchlight) as i32);
        (Rgb::splat(global_light) * sunlight_intensity + Rgb::splat(local_light)).saturate()
    }

    fn ao_factor(ao: u8) -> f32 {
        let shading = &CONFIG.shading;
        let ao_factor_min = shading.ao_factor_min;
        let ao_factor_max = shading.ao_factor_max;
        let ao_max = 3.0;

        1.0 - lerp(ao_factor_min, ao_factor_max, ao as f32 / ao_max)
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(super) struct UniformData {
    side_factors: [f32; 4],
}

impl Default for UniformData {
    fn default() -> Self {
        Self {
            side_factors: CONFIG.shading.side_factors.inner().into_array(),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    side_factors: EnumMap<SideShade, f32>,
    ao_factor_min: f32,
    ao_factor_max: f32,
    light_attenuation: f32,
}

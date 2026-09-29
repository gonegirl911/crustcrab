use crate::{
    client::{
        CLIENT_CONFIG,
        renderer::{Renderer, buffer::MemoryState, texture::data::DataTexture, uniform::Uniform},
    },
    server::game::world::block::{BlockLight, data::SideShade},
    shared::{
        color::{Rgb, Rgba},
        enum_map::EnumMap,
        quantize, utils,
    },
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use std::{array, sync::LazyLock};

pub struct Shading {
    pub mut(self) uniform: Uniform<ShadingUniformData>,
    pub mut(self) texture: DataTexture,
    uploaded_nightness: Option<f32>,
}

impl Shading {
    pub fn new(renderer: &Renderer) -> Self {
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
                .format(wgpu::TextureFormat::Rgba16Unorm)
                .visibility(wgpu::ShaderStages::VERTEX)
                .filterable(false)
                .build(),
            uploaded_nightness: None,
        }
    }

    pub fn update(&mut self, renderer: &Renderer, nightness: f32) {
        if self.uploaded_nightness.replace(nightness) != Some(nightness) {
            let table = LightTable::new(nightness);
            self.texture.write(renderer, table.as_slice());
        }
    }
}

pub struct LightTable([[[Rgba<u16>; Self::AO_LEVELS]; Self::LIGHT_LEVELS]; Self::LIGHT_LEVELS]);

impl LightTable {
    const AO_LEVELS: usize = 4;
    const LIGHT_LEVELS: usize = BlockLight::COMPONENT_MAX as usize + 1;

    fn new(nightness: f32) -> Self {
        let sunlight_intensity = CLIENT_CONFIG.sky.sunlight_intensity(nightness);
        Self(array::from_fn(|torchlight| {
            array::from_fn(|skylight| {
                array::from_fn(|ao| {
                    let value =
                        Self::world_light(skylight as u8, torchlight as u8, sunlight_intensity)
                            * Self::ao_factor(ao as u8);
                    value.with_alpha(1.0).map(quantize::to_unorm16)
                })
            })
        }))
    }

    pub fn value(&self, light: BlockLight, ao: u8) -> Rgb<f32> {
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
        let light_attenuation = CLIENT_CONFIG.shading.light_attenuation;
        let light_max = BlockLight::COMPONENT_MAX;

        let global_light = light_attenuation.powi((light_max - skylight) as i32);
        let local_light = light_attenuation.powi((light_max - torchlight) as i32);
        (Rgb::splat(global_light) * sunlight_intensity + Rgb::splat(local_light)).saturate()
    }

    fn ao_factor(ao: u8) -> f32 {
        let shading = &CLIENT_CONFIG.shading;
        let ao_factor_min = shading.ao_factor_min;
        let ao_factor_max = shading.ao_factor_max;
        let ao_max = 3.0;

        1.0 - utils::lerp(ao_factor_min, ao_factor_max, ao as f32 / ao_max)
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct ShadingUniformData {
    side_factors: [f32; 4],
}

impl Default for ShadingUniformData {
    fn default() -> Self {
        Self {
            side_factors: CLIENT_CONFIG.shading.side_factors.inner().into_array(),
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

pub static DAY_LIGHT_TABLE: LazyLock<LightTable> = LazyLock::new(|| LightTable::new(0.0));

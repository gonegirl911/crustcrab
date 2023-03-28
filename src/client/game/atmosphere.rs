use crate::{
    client::renderer::{Renderer, Uniform},
    color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use std::fs;

pub struct Atmosphere(Uniform<AtmosphereUniformData>);

impl Atmosphere {
    pub fn new(renderer: &Renderer) -> Self {
        Self(Uniform::with_constant_data(
            renderer,
            &AtmosphereUniformData::new(),
            wgpu::ShaderStages::FRAGMENT,
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
struct AtmosphereUniformData {
    sun_intensity: Float3,
    sc_air: Float3,
    sc_haze: Float3,
    ex_air: Float3,
    ex_haze: Float3,
    ex: Rgb<f32>,
    s_air: f32,
    s_haze: f32,
    g: f32,
    padding: [f32; 2],
}

impl AtmosphereUniformData {
    fn new() -> Self {
        let settings = AtmosphereSettings::new();
        Self {
            sun_intensity: settings.sun_intensity().into(),
            sc_air: settings.sc_air.into(),
            sc_haze: settings.sc_haze.into(),
            ex_air: settings.ex_air().into(),
            ex_haze: settings.ex_haze().into(),
            ex: settings.ex(),
            s_air: settings.s_air,
            s_haze: settings.s_haze,
            g: settings.g,
            padding: [0.0; 2],
        }
    }
}

#[derive(Deserialize)]
struct AtmosphereSettings {
    sun_intensity: Rgb<f32>,
    ab_air: Rgb<f32>,
    ab_haze: Rgb<f32>,
    sc_air: Rgb<f32>,
    sc_haze: Rgb<f32>,
    s_air: f32,
    s_haze: f32,
    g: f32,
}

impl AtmosphereSettings {
    fn new() -> Self {
        toml::from_str(&fs::read_to_string("assets/atmosphere.toml").expect("file should exist"))
            .expect("file should be valid")
    }

    fn sun_intensity(&self) -> Rgb<f32> {
        self.sun_intensity * (-self.ex_air() * self.s_air - self.ex_haze() * self.s_haze).exp()
    }

    fn ex_air(&self) -> Rgb<f32> {
        self.ab_air + self.sc_air
    }

    fn ex_haze(&self) -> Rgb<f32> {
        self.ab_haze + self.sc_haze
    }

    fn ex(&self) -> Rgb<f32> {
        self.ex_air() + self.ex_haze()
    }
}
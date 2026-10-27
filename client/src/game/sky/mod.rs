mod atmosphere;
mod object;
mod star;

use super::clock::RenderTime;
use crate::{
    CONFIG,
    renderer::{
        Renderer, buffer::MemoryState, render_pass::color_pass, types::Float3, uniform::Uniform,
    },
};
use atmosphere::Atmosphere;
use base::{
    color::Rgb,
    math::interpolation::{lerp, ramp},
};
use bytemuck::{Pod, Zeroable};
use serde::Deserialize;

pub(super) struct Sky {
    atmosphere: Atmosphere,
    stars: star::Dome,
    objects: object::Set,
    pub(super) mut(self) uniform: Uniform<UniformData>,
}

impl Sky {
    pub(super) fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(
            renderer,
            MemoryState::UNINIT,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        );
        let atmosphere = Atmosphere::new(
            renderer,
            player_uniform_bind_group_layout,
            &uniform.bind_group_layout,
        );
        let stars = star::Dome::new(renderer, player_uniform_bind_group_layout);
        let objects = object::Set::new(
            renderer,
            player_uniform_bind_group_layout,
            &uniform.bind_group_layout,
        );
        Self {
            atmosphere,
            stars,
            objects,
            uniform,
        }
    }

    #[rustfmt::skip]
    pub(super) fn draw(
        &self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_uniform_bind_group: &wgpu::BindGroup,
        time: &RenderTime,
    ) {
        self.uniform.set(renderer, &UniformData::new(time));

        let mut render_pass = color_pass(view, encoder, wgpu::LoadOp::Clear(Default::default()));
        self.atmosphere.draw(
            &mut render_pass,
            player_uniform_bind_group,
            &self.uniform.bind_group,
        );
        self.stars.draw(&mut render_pass, player_uniform_bind_group, time);
        self.objects.draw(
            &mut render_pass,
            player_uniform_bind_group,
            &self.uniform.bind_group,
            time,
        );
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub(super) struct UniformData {
    sun_dir: Float3,
    color: Float3,
    horizon_color: Rgb<f32>,
    glow_opacity: f32,
    glow_color: Rgb<f32>,
    arc_angle: f32,
    sunlight_intensity: Float3,
}

impl UniformData {
    fn new(time @ &RenderTime { nightness, .. }: &RenderTime) -> Self {
        Self {
            sun_dir: time.sun_dir().into(),
            color: CONFIG.sky.color(nightness).into(),
            horizon_color: CONFIG.sky.horizon_color(nightness),
            glow_opacity: Self::glow_opacity(nightness),
            glow_color: CONFIG.sky.glow_color(nightness),
            arc_angle: CONFIG.sky.arc_angle(nightness).to_radians(),
            sunlight_intensity: CONFIG.sky.sunlight_intensity(nightness).into(),
        }
    }

    fn glow_opacity(nightness: f32) -> f32 {
        const GLOW_CENTER: f32 = 0.5;
        const GLOW_BANDWIDTH: f32 = 0.5;

        1.0 - ((nightness - GLOW_CENTER) / GLOW_BANDWIDTH).powi(2)
    }
}

#[derive(Deserialize)]
pub(crate) struct Config {
    day: TimePhaseConfig,
    night: TimePhaseConfig,
    star: star::Config,
    object: object::Config,
}

impl Config {
    fn color(&self, nightness: f32) -> Rgb<f32> {
        lerp(self.day.color, self.night.color, nightness)
    }

    fn horizon_color(&self, nightness: f32) -> Rgb<f32> {
        lerp(self.day.horizon_color, self.night.horizon_color, nightness)
    }

    fn glow_color(&self, nightness: f32) -> Rgb<f32> {
        lerp(self.day.glow_color, self.night.glow_color, nightness)
    }

    fn arc_angle(&self, nightness: f32) -> f32 {
        lerp(
            self.day.arc_angle,
            self.night.arc_angle,
            ramp(1.0 / 3.0, 2.0 / 3.0, nightness) * ramp(1.0, 2.0 / 3.0, nightness),
        )
    }

    pub(in crate::game) fn sunlight_intensity(&self, nightness: f32) -> Rgb<f32> {
        lerp(
            self.day.sunlight_intensity,
            self.night.sunlight_intensity,
            nightness,
        )
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    color: Rgb<f32>,
    horizon_color: Rgb<f32>,
    glow_color: Rgb<f32>,
    arc_angle: f32,
    sunlight_intensity: Rgb<f32>,
}

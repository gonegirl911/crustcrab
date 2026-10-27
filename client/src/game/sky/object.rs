use crate::{
    CONFIG,
    game::clock::RenderTime,
    renderer::{
        self, Immediates as _, Renderer,
        post_process::PostProcessor,
        render_pipeline::{RenderPipeline, read_wgsl},
        texture::image::{ImageTextureArray, load_rgba},
        types::Float3,
    },
};
use base::math::interpolation::lerp;
use bytemuck::{Pod, Zeroable};
use nalgebra::Vector3;
use serde::Deserialize;

pub(super) struct Set {
    textures: ImageTextureArray,
    render_pipeline: RenderPipeline,
}

impl Set {
    pub(super) fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        sky_uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let textures = ImageTextureArray::builder()
            .renderer(renderer)
            .images([
                &load_rgba("client/assets/textures/sky/sun.png"),
                &load_rgba("client/assets/textures/sky/moon.png"),
            ])
            .is_srgb(true)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("client/assets/shaders/object.wgsl"))
            .bind_group_layouts(&[
                Some(player_uniform_bind_group_layout),
                Some(sky_uniform_bind_group_layout),
                Some(&textures.bind_group_layout),
            ])
            .immediate_size(Immediates::SIZE)
            .format(PostProcessor::FORMAT)
            .build();
        Self {
            textures,
            render_pipeline,
        }
    }

    pub(super) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_uniform_bind_group: &wgpu::BindGroup,
        sky_uniform_bind_group: &wgpu::BindGroup,
        time @ &RenderTime { nightness, .. }: &RenderTime,
    ) {
        let sun_dir = time.sun_dir();
        let up = -sun_dir.x.signum() * Vector3::y();
        let sun_imm = Immediates::new(0, sun_dir, up, nightness);
        let moon_imm = Immediates::new(1, -sun_dir, up, nightness);

        self.render_pipeline.bind(
            render_pass,
            [
                player_uniform_bind_group,
                sky_uniform_bind_group,
                &self.textures.bind_group,
            ],
        );
        sun_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
        moon_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Immediates {
    dir: Float3,
    up: Vector3<f32>,
    size: f32,
    tex_index: u32,
    brightness: f32,
    padding: [f32; 2],
}

impl Immediates {
    fn new(tex_index: u32, dir: Vector3<f32>, up: Vector3<f32>, nightness: f32) -> Self {
        Self {
            dir: dir.into(),
            up,
            size: CONFIG.sky.object.size,
            tex_index,
            brightness: CONFIG.sky.object.brightness(nightness),
            padding: Default::default(),
        }
    }
}

impl renderer::Immediates for Immediates {}

#[derive(Deserialize)]
pub(super) struct Config {
    size: f32,
    day: TimePhaseConfig,
    night: TimePhaseConfig,
}

impl Config {
    fn brightness(&self, nightness: f32) -> f32 {
        lerp(self.day.brightness, self.night.brightness, nightness)
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    brightness: f32,
}

use crate::{
    client::{
        CLIENT_CONFIG,
        game::clock::RenderTime,
        renderer::{
            Renderer, Surface,
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            texture::image::ImageTextureArray,
            utils::{Immediates, load_rgba, read_wgsl},
        },
    },
    shared::{color::Float3, utils},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Vector3;
use serde::Deserialize;

pub struct ObjectSet {
    textures: ImageTextureArray,
    render_pipeline: RenderPipeline,
}

impl ObjectSet {
    pub fn new(
        renderer: &Renderer,
        surface: &Surface,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let textures = ImageTextureArray::builder()
            .renderer(renderer)
            .surface(surface)
            .images([
                &load_rgba("assets/textures/sky/sun.png"),
                &load_rgba("assets/textures/sky/moon.png"),
            ])
            .is_srgb(true)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/object.wgsl"))
            .bind_group_layouts(&[
                player_bind_group_layout,
                sky_bind_group_layout,
                textures.bind_group_layout(),
            ])
            .immediate_size(ObjectImmediates::SIZE)
            .format(PostProcessor::FORMAT)
            .build();
        Self {
            textures,
            render_pipeline,
        }
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        time: RenderTime,
    ) {
        let sun_dir = time.sun_dir();
        let up = -sun_dir.x.signum() * Vector3::y();
        let nightness = time.nightness();
        let sun_imm = ObjectImmediates::new(0, sun_dir, up, nightness);
        let moon_imm = ObjectImmediates::new(1, -sun_dir, up, nightness);

        self.render_pipeline.bind(
            render_pass,
            [
                player_bind_group,
                sky_bind_group,
                self.textures.bind_group(),
            ],
        );
        sun_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
        moon_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectImmediates {
    dir: Float3,
    up: Vector3<f32>,
    size: f32,
    tex_index: u32,
    brightness: f32,
    padding: [f32; 2],
}

impl ObjectImmediates {
    fn new(tex_index: u32, dir: Vector3<f32>, up: Vector3<f32>, nightness: f32) -> Self {
        Self {
            dir: dir.into(),
            up,
            size: CLIENT_CONFIG.sky.object.size,
            tex_index,
            brightness: CLIENT_CONFIG.sky.object.brightness(nightness),
            padding: Default::default(),
        }
    }
}

impl Immediates for ObjectImmediates {}

#[derive(Deserialize)]
pub struct ObjectConfig {
    size: f32,
    day: TimePhaseConfig,
    night: TimePhaseConfig,
}

impl ObjectConfig {
    fn brightness(&self, nightness: f32) -> f32 {
        utils::lerp(self.day.brightness, self.night.brightness, nightness)
    }
}

#[derive(Deserialize)]
struct TimePhaseConfig {
    brightness: f32,
}

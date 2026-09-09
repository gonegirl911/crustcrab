use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer, Surface,
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            texture::image::ImageTextureArray,
            utils::{Immediates, billboard, load_rgba, read_wgsl},
        },
    },
    server::{ServerEvent, game::clock::Time},
    shared::utils,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Vector3, vector};
use serde::Deserialize;

pub struct ObjectSet {
    textures: ImageTextureArray,
    render_pipeline: RenderPipeline,
    sun_imm: ObjectImmediates,
    moon_imm: ObjectImmediates,
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
                load_rgba("assets/textures/sky/sun.png"),
                load_rgba("assets/textures/sky/moon.png"),
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
        let (sun_imm, moon_imm) = Self::imm(Default::default());
        Self {
            textures,
            render_pipeline,
            sun_imm,
            moon_imm,
        }
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
    ) {
        self.render_pipeline.bind(
            render_pass,
            [
                player_bind_group,
                sky_bind_group,
                self.textures.bind_group(),
            ],
        );
        self.sun_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
        self.moon_imm.set(render_pass);
        render_pass.draw(0..6, 0..1);
    }

    fn imm(time: Time) -> (ObjectImmediates, ObjectImmediates) {
        let sun_dir = time.sun_dir();
        let up = -sun_dir.x.signum() * Vector3::y();
        let nightness = time.nightness();
        (
            ObjectImmediates::new(0, sun_dir, up, nightness),
            ObjectImmediates::new(1, -sun_dir, up, nightness),
        )
    }
}

impl EventHandler for ObjectSet {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::ServerEvent(ServerEvent::TimeUpdated(time)) = *event {
            (self.sun_imm, self.moon_imm) = Self::imm(time);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectImmediates {
    m: Matrix4<f32>,
    tex_index: u32,
    brightness: f32,
}

impl ObjectImmediates {
    #[rustfmt::skip]
    fn new(tex_index: u32, dir: Vector3<f32>, up: Vector3<f32>, nightness: f32) -> Self {
        let config = &CLIENT_CONFIG.sky.object;
        Self {
            m: billboard(dir.into(), Point3::origin(), up)
                .prepend_nonuniform_scaling(&vector![config.size, config.size, 1.0]),
            tex_index,
            brightness: config.brightness(nightness),
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

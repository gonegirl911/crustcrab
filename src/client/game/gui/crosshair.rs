use super::Gui;
use crate::client::{
    CLIENT_CONFIG,
    event_loop::{Event, EventHandler},
    renderer::{
        Renderer, Surface,
        buffer::MemoryState,
        effect::PostProcessor,
        render_pipeline::RenderPipeline,
        texture::image::ImageTexture,
        uniform::Uniform,
        utils::{load_rgba, read_wgsl},
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector2};
use serde::Deserialize;

pub struct Crosshair {
    uniform: Uniform<CrosshairUniformData>,
    texture: ImageTexture,
    render_pipeline: RenderPipeline,
}

impl Crosshair {
    pub fn new(
        renderer: &Renderer,
        surface: &Surface,
        input_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let texture = ImageTexture::builder()
            .renderer(renderer)
            .surface(surface)
            .image(load_rgba("assets/textures/gui/crosshair.png"))
            .is_srgb(false)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/crosshair.wgsl"))
            .bind_group_layouts(&[
                uniform.bind_group_layout(),
                texture.bind_group_layout(),
                input_bind_group_layout,
            ])
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState::ALPHA_BLENDING)
            .build();
        Self {
            uniform,
            texture,
            render_pipeline,
        }
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.render_pipeline.bind(
            render_pass,
            [
                self.uniform.bind_group(),
                self.texture.bind_group(),
                input_bind_group,
            ],
        );
        render_pass.draw(0..6, 0..1);
    }
}

impl EventHandler for Crosshair {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, _: &Event, (renderer, surface): Self::Context<'_>) {
        if surface.is_resized {
            self.uniform
                .set(renderer, &CrosshairUniformData::new(surface));
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CrosshairUniformData {
    transform: Matrix4<f32>,
}

impl CrosshairUniformData {
    fn new(surface: &Surface) -> Self {
        let scaling = Gui::scaling(surface, CLIENT_CONFIG.gui.crosshair.size);
        let transform = Gui::transform(scaling, Vector2::repeat(0.5));
        Self { transform }
    }
}

#[derive(Deserialize)]
pub struct CrosshairConfig {
    size: f32,
}

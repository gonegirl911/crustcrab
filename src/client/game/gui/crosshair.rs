use super::Gui;
use crate::client::{
    CLIENT_CONFIG,
    event_loop::{Event, EventHandler},
    renderer::{
        Renderer,
        buffer::MemoryState,
        effect::PostProcessor,
        program::Program,
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
    program: Program,
}

impl Crosshair {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let texture = ImageTexture::builder()
            .renderer(renderer)
            .image(load_rgba("assets/textures/gui/crosshair.png"))
            .is_srgb(false)
            .build();
        let program = Program::builder()
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
            program,
        }
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, input_bind_group: &wgpu::BindGroup) {
        self.program.bind(
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
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, _: &Event, renderer: Self::Context<'_>) {
        if renderer.is_surface_resized {
            self.uniform
                .set(renderer, &CrosshairUniformData::new(renderer));
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CrosshairUniformData {
    transform: Matrix4<f32>,
}

impl CrosshairUniformData {
    fn new(renderer: &Renderer) -> Self {
        Self {
            transform: Gui::transform(
                Gui::scaling(renderer, CLIENT_CONFIG.gui.crosshair.size),
                Vector2::repeat(0.5),
            ),
        }
    }
}

#[derive(Deserialize)]
pub struct CrosshairConfig {
    size: f32,
}

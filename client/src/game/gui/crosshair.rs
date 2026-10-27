use super::Gui;
use crate::{
    CONFIG,
    event::{self, Event},
    renderer::{
        Renderer,
        buffer::MemoryState,
        post_process::PostProcessor,
        render_pipeline::{RenderPipeline, read_wgsl},
        surface::Surface,
        texture::image::{ImageTexture, load_rgba},
        uniform::Uniform,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector2};
use serde::Deserialize;

pub(super) struct Crosshair {
    uniform: Uniform<UniformData>,
    texture: ImageTexture,
    render_pipeline: RenderPipeline,
}

impl Crosshair {
    pub(super) fn new(renderer: &Renderer) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let texture = ImageTexture::builder()
            .renderer(renderer)
            .image(&load_rgba("client/assets/textures/gui/crosshair.png"))
            .is_srgb(false)
            .build();
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("client/assets/shaders/crosshair.wgsl"))
            .bind_group_layouts(&[
                Some(&uniform.bind_group_layout),
                Some(&texture.bind_group_layout),
            ])
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::OneMinusDst,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            })
            .build();
        Self {
            uniform,
            texture,
            render_pipeline,
        }
    }

    pub(super) fn draw(&self, render_pass: &mut wgpu::RenderPass) {
        self.render_pipeline.bind(
            render_pass,
            [&self.uniform.bind_group, &self.texture.bind_group],
        );
        render_pass.draw(0..6, 0..1);
    }
}

impl event::Handler for Crosshair {
    type Context<'a> = (&'a Renderer, &'a Surface);

    fn handle(&mut self, _: &Event, (renderer, surface): Self::Context<'_>) {
        if surface.is_resized() {
            self.uniform.set(
                renderer,
                &UniformData::new(surface, CONFIG.gui.crosshair.size),
            );
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct UniformData {
    transform: Matrix4<f32>,
}

impl UniformData {
    fn new(surface: &Surface, size: f32) -> Self {
        let size = Gui::unit(surface) * size;
        let transform = Gui::transform(size, Vector2::repeat(0.5));
        Self { transform }
    }
}

#[derive(Deserialize)]
pub(super) struct Config {
    size: f32,
}

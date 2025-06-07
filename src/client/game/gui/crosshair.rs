use crate::client::{
    CLIENT_CONFIG,
    event_loop::{Event, EventHandler},
    game::gui::Gui,
    renderer::{
        Renderer, buffer::MemoryState, effect::PostProcessor, program::Program,
        texture::image::ImageTexture, uniform::Uniform,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Vector2};
use serde::Deserialize;
use winit::event::WindowEvent;

pub struct Crosshair {
    uniform: Uniform<CrosshairUniformData>,
    texture: ImageTexture,
    program: Program,
}

impl Crosshair {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer, MemoryState::UNINIT, wgpu::ShaderStages::VERTEX);
        let texture = ImageTexture::new(
            renderer,
            "assets/textures/gui/crosshair.png",
            1,
            false,
            Default::default(),
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/crosshair.wgsl"),
            &[],
            &[
                uniform.bind_group_layout(),
                texture.bind_group_layout(),
                input_bind_group_layout,
            ],
            &[],
            None,
            None,
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
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

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        if matches!(event, Event::WindowEvent(WindowEvent::RedrawRequested)) && renderer.is_resized
        {
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

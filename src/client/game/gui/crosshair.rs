use super::Gui;
use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{
        effect::PostProcessor, program::Program, texture::image::ImageTexture, uniform::Uniform,
        Renderer,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Matrix4;
use std::mem;
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Crosshair {
    uniform: Uniform<CrosshairUniformData>,
    texture: ImageTexture,
    program: Program,
    is_resized: bool,
}

impl Crosshair {
    pub fn new(renderer: &Renderer, input_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::uninit_mut(renderer, wgpu::ShaderStages::VERTEX);
        let texture = ImageTexture::new(
            renderer,
            "assets/textures/gui/crosshair.png",
            false,
            true,
            1,
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
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            None,
            None,
        );
        Self {
            uniform,
            texture,
            program,
            is_resized: true,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        input_bind_group: &'a wgpu::BindGroup,
    ) {
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

    fn transform(renderer: &Renderer) -> Matrix4<f32> {
        Gui::viewport(renderer) * Gui::element_scaling(Gui::element_size(renderer, 1.0))
    }
}

impl EventHandler for Crosshair {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: PhysicalSize { width, height },
                        ..
                    },
                ..
            } if *width != 0 && *height != 0 => {
                self.is_resized = true;
            }
            Event::MainEventsCleared => {
                if mem::take(&mut self.is_resized) {
                    self.uniform.set(
                        renderer,
                        &CrosshairUniformData::new(Self::transform(renderer)),
                    );
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CrosshairUniformData {
    transform: Matrix4<f32>,
}

impl CrosshairUniformData {
    fn new(transform: Matrix4<f32>) -> Self {
        Self { transform }
    }
}

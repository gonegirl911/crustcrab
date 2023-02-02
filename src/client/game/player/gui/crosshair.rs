use crate::client::{
    event_loop::{Event, EventHandler},
    renderer::{ImageTexture, Program, Renderer, Uniform},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Matrix4};
use winit::{dpi::PhysicalSize, event::WindowEvent};

pub struct Crosshair {
    uniform: Uniform<CrosshairUniformData>,
    texture: ImageTexture,
    program: Program,
    is_resized: bool,
}

impl Crosshair {
    pub fn new(renderer: &Renderer, output_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let uniform = Uniform::new(renderer);
        let texture = ImageTexture::new(
            renderer,
            include_bytes!("../../../../../assets/textures/crosshair.png"),
            true,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../../assets/shaders/crosshair.wgsl"),
            &[],
            &[
                uniform.bind_group_layout(),
                texture.bind_group_layout(),
                output_bind_group_layout,
            ],
            &[],
            None,
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
        output_bind_group: &'a wgpu::BindGroup,
    ) {
        self.program.draw(
            render_pass,
            [
                self.uniform.bind_group(),
                self.texture.bind_group(),
                output_bind_group,
            ],
        );
        render_pass.draw(0..6, 0..1);
    }
}

impl EventHandler for Crosshair {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer @ Renderer { config, .. }: Self::Context<'_>) {
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
            Event::RedrawRequested(_) if self.is_resized => {
                let size = (config.height as f32 * 0.065).max(36.0);
                self.uniform.update(
                    renderer,
                    &CrosshairUniformData::new(Matrix4::new_nonuniform_scaling(&vector![
                        size / config.width as f32,
                        size / config.height as f32,
                        1.0
                    ])),
                );
            }
            Event::RedrawEventsCleared => {
                self.is_resized = false;
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

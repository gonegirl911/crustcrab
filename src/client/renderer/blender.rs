use super::{
    program::{Program, PushConstants},
    texture::screen::ScreenTexture,
    Renderer,
};
use crate::client::event_loop::{Event, EventHandler};
use bytemuck::{Pod, Zeroable};
use std::mem;

pub struct Blender {
    texture: ScreenTexture,
    program: Program,
}

impl Blender {
    pub fn new(renderer: &Renderer, format: wgpu::TextureFormat) -> Self {
        let texture = ScreenTexture::new(renderer, format);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/blender.wgsl"),
            &[],
            &[texture.bind_group_layout()],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::FRAGMENT,
                range: 0..mem::size_of::<BlenderPushConstants>() as u32,
            }],
            format,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            None,
            None,
        );
        Self { texture, program }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        self.texture.view()
    }

    #[rustfmt::skip]
    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder, opacity: f32) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.program.bind(&mut render_pass, [self.texture.bind_group()]);
        BlenderPushConstants::new(opacity).set(&mut render_pass);
        render_pass.draw(0..3, 0..1);
    }
}

impl EventHandler for Blender {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        self.texture.handle(event, renderer);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlenderPushConstants {
    opacity: f32,
}

impl BlenderPushConstants {
    fn new(opacity: f32) -> Self {
        Self { opacity }
    }
}

impl PushConstants for BlenderPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::FRAGMENT;
}

use crate::{
    client::renderer::{ImageTexture, PostProcessor, Program, Renderer, Uniform},
    color::{Float3, Rgb},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Matrix4;
use std::mem;

pub struct Sky {
    sun: Object,
    moon: Object,
    uniform: Uniform<SkyUniformData>,
}

impl Sky {
    const COLOR: Rgb<f32> = Rgb::splat(0.0);
    const LIGHT_INTENSITY: Rgb<f32> = Rgb::new(0.15, 0.15, 0.3);

    pub fn new(renderer: &Renderer) -> Self {
        Self {
            sun: Object::new(
                renderer,
                include_bytes!("../../../assets/textures/sun.png"),
                true,
            ),
            moon: Object::new(
                renderer,
                include_bytes!("../../../assets/textures/moon.png"),
                true,
            ),
            uniform: Uniform::with_constant_data(
                renderer,
                &SkyUniformData::new(Self::COLOR, Self::LIGHT_INTENSITY),
                wgpu::ShaderStages::VERTEX_FRAGMENT,
            ),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(&self, view: &wgpu::TextureView, encoder: &mut wgpu::CommandEncoder) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(Self::COLOR.into()),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        // self.sun.draw(&mut render_pass, todo!());
        // self.moon.draw(&mut render_pass, todo!());
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    color: Float3,
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(color: Rgb<f32>, light_intensity: Rgb<f32>) -> Self {
        Self {
            color: color.into(),
            light_intensity: light_intensity.into(),
        }
    }
}

pub struct Object {
    texture: ImageTexture,
    program: Program,
}

impl Object {
    pub fn new(renderer: &Renderer, bytes: &[u8], is_srgb: bool) -> Self {
        let texture = ImageTexture::new(renderer, bytes, is_srgb, true, 1);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/object.wgsl"),
            &[],
            &[texture.bind_group_layout()],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..mem::size_of::<ObjectPushConstants>() as u32,
            }],
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(wgpu::Face::Back),
            None,
        );
        Self { texture, program }
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, mvp: Matrix4<f32>) {
        self.program.bind(render_pass, [self.texture.bind_group()]);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&[ObjectPushConstants::new(mvp)]),
        );
        render_pass.draw(0..6, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectPushConstants {
    mvp: Matrix4<f32>,
}

impl ObjectPushConstants {
    fn new(mvp: Matrix4<f32>) -> Self {
        Self { mvp }
    }
}

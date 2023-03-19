use super::player::Player;
use crate::client::renderer::{ImageTexture, PostProcessor, Program, Renderer};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3};
use std::mem;

pub struct Objects {
    sun: Object,
    moon: Object,
}

impl Objects {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            sun: Object::new(
                renderer,
                player_bind_group_layout,
                include_bytes!("../../../assets/textures/sun.png"),
                true,
            ),
            moon: Object::new(
                renderer,
                player_bind_group_layout,
                include_bytes!("../../../assets/textures/moon.png"),
                true,
            ),
        }
    }

    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sun_coords: Point3<f32>,
    ) {
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
        self.sun.draw(
            &mut render_pass,
            player_bind_group,
            Self::sun_m(sun_coords),
        );
        self.moon.draw(
            &mut render_pass,
            player_bind_group,
            Self::moon_m(sun_coords),
        );
    }

    fn sun_m(sun_coords: Point3<f32>) -> Matrix4<f32> {
        Self::m(sun_coords).prepend_scaling(0.5)
    }

    fn moon_m(sun_coords: Point3<f32>) -> Matrix4<f32> {
        Self::m(-sun_coords).prepend_scaling(0.5)
    }

    fn m(coords: Point3<f32>) -> Matrix4<f32> {
        Matrix4::face_towards(&coords, &(coords * 2.0), &Player::WORLD_UP)
    }
}

struct Object {
    texture: ImageTexture,
    program: Program,
}

impl Object {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        image: &[u8],
        is_srgb: bool,
    ) -> Self {
        let texture = ImageTexture::new(renderer, image, is_srgb, true, 1);
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/object.wgsl"),
            &[],
            &[player_bind_group_layout, texture.bind_group_layout()],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..mem::size_of::<ObjectPushConstants>() as u32,
            }],
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
            None,
            None,
        );
        Self { texture, program }
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        m: Matrix4<f32>,
    ) {
        self.program.bind(
            render_pass,
            [player_bind_group, self.texture.bind_group()],
        );
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&[ObjectPushConstants::new(m)]),
        );
        render_pass.draw(0..6, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectPushConstants {
    m: Matrix4<f32>,
}

impl ObjectPushConstants {
    fn new(m: Matrix4<f32>) -> Self {
        Self { m }
    }
}

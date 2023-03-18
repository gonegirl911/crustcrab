use super::player::Player;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{ImageTexture, PostProcessor, Program, Renderer, Uniform},
    },
    color::{Float3, Rgb},
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Matrix4, Point3};
use std::{f32::consts::TAU, mem};

pub struct Sky {
    sun: Object,
    moon: Object,
    uniform: Uniform<SkyUniformData>,
    sun_coords: Point3<f32>,
    updated_time: Option<f32>,
}

impl Sky {
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
            uniform: Uniform::new(renderer, wgpu::ShaderStages::VERTEX_FRAGMENT),
            sun_coords: Default::default(),
            updated_time: Some(0.0),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        self.sun.draw(&mut render_pass, player_bind_group, self.sun_m());
        self.moon.draw(&mut render_pass, player_bind_group, self.moon_m());
    }

    fn sun_m(&self) -> Matrix4<f32> {
        Self::m(self.sun_coords).prepend_scaling(0.5)
    }

    fn moon_m(&self) -> Matrix4<f32> {
        Self::m(-self.sun_coords).prepend_scaling(0.5)
    }

    fn sun_coords(time: f32) -> Point3<f32> {
        let theta = time * TAU;
        point![theta.cos(), theta.sin(), 0.0]
    }

    fn m(coords: Point3<f32>) -> Matrix4<f32> {
        Matrix4::face_towards(&coords, &(coords * 2.0), &Player::WORLD_UP)
    }
}

impl EventHandler for Sky {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated { time }) => {
                self.updated_time = Some(*time);
            }
            Event::RedrawRequested(_) => {
                if let Some(time) = self.updated_time {
                    self.sun_coords = Self::sun_coords(time);
                    self.uniform.write(
                        renderer,
                        &SkyUniformData::new(
                            Rgb::splat(0.0),
                            self.sun_coords,
                            Rgb::new(0.15, 0.15, 0.3),
                        ),
                    );
                }
            }
            Event::RedrawEventsCleared => {
                self.updated_time = None;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    color: Float3,
    sun_coords: Float3,
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(color: Rgb<f32>, sun_coords: Point3<f32>, light_intensity: Rgb<f32>) -> Self {
        Self {
            color: color.into(),
            sun_coords: sun_coords.into(),
            light_intensity: light_intensity.into(),
        }
    }
}

pub struct Object {
    texture: ImageTexture,
    program: Program,
}

impl Object {
    pub fn new(
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
            Some(wgpu::Face::Back),
            None,
        );
        Self { texture, program }
    }

    #[rustfmt::skip]
    pub fn draw<'a>(
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

use super::player::Player;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            effect::PostProcessor, program::Program, texture::image::ImageTextureArray,
            uniform::Uniform, Renderer,
        },
    },
    server::{
        game::clock::{Stage, Time},
        ServerEvent,
    },
    shared::{
        color::{Float3, Rgb},
        utils,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Matrix4, Point3, Vector3};
use std::mem;

pub struct Sky {
    objects: Objects,
    uniform: Uniform<SkyUniformData>,
    time: Result<Time, Time>,
}

impl Sky {
    const DAY_INTENSITY: Rgb<f32> = Rgb::splat(1.0);
    const NIGHT_INTENSITY: Rgb<f32> = Rgb::new(0.15, 0.15, 0.3);

    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            objects: Objects::new(renderer, player_bind_group_layout),
            uniform: Uniform::uninit_mut(renderer, wgpu::ShaderStages::VERTEX),
            time: Err(Default::default()),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.uniform.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.uniform.bind_group()
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
    ) {
        self.objects.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Default::default()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            }),
            player_bind_group,
            self.time.unwrap_or_else(|_| unreachable!()),
        );
    }

    fn light_intensity(stage: Stage) -> Rgb<f32> {
        match stage {
            Stage::Dawn { progress } => {
                utils::lerp(Self::NIGHT_INTENSITY, Self::DAY_INTENSITY, progress)
            }
            Stage::Day => Self::DAY_INTENSITY,
            Stage::Dusk { progress } => {
                utils::lerp(Self::DAY_INTENSITY, Self::NIGHT_INTENSITY, progress)
            }
            Stage::Night => Self::NIGHT_INTENSITY,
        }
    }
}

impl EventHandler for Sky {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.time = Err(*time);
            }
            Event::MainEventsCleared => {
                if let Err(time) = self.time {
                    self.uniform.set(
                        renderer,
                        &SkyUniformData::new(Self::light_intensity(time.stage())),
                    );
                    self.time = Ok(time);
                }
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SkyUniformData {
    light_intensity: Float3,
}

impl SkyUniformData {
    fn new(light_intensity: Rgb<f32>) -> Self {
        Self {
            light_intensity: light_intensity.into(),
        }
    }
}

struct Objects {
    textures: ImageTextureArray,
    program: Program,
}

impl Objects {
    fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let textures = ImageTextureArray::new(
            renderer,
            [
                "assets/textures/sky/sun.png",
                "assets/textures/sky/moon.png",
            ],
            true,
            true,
            1,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/object.wgsl"),
            &[],
            &[player_bind_group_layout, textures.bind_group_layout()],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
                range: 0..mem::size_of::<ObjectsPushConstants>() as u32,
            }],
            PostProcessor::FORMAT,
            None,
            None,
            None,
        );
        Self { textures, program }
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        time: Time,
    ) {
        self.program.bind(render_pass, [player_bind_group, self.textures.bind_group()]);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::cast_slice(&[ObjectsPushConstants::sun(time)]),
        );
        render_pass.draw(0..6, 0..1);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            0,
            bytemuck::cast_slice(&[ObjectsPushConstants::moon(time)]),
        );
        render_pass.draw(0..6, 0..1);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct ObjectsPushConstants {
    transform: Matrix4<f32>,
    tex_idx: u32,
    brightness: f32,
}

impl ObjectsPushConstants {
    const SIZE: f32 = 0.15;
    const SUN_BRIGHTNESS: f32 = 15.0;

    fn sun(time: Time) -> Self {
        Self::new(
            time.sun_dir(),
            Self::SIZE,
            0,
            Self::sun_brightness(time.stage()),
        )
    }

    fn moon(time: Time) -> Self {
        Self::new(time.moon_dir(), Self::SIZE, 1, 1.0)
    }

    fn new(dir: Vector3<f32>, size: f32, tex_idx: u32, brightness: f32) -> Self {
        Self {
            transform: Matrix4::face_towards(&dir.into(), &Point3::origin(), &Player::WORLD_UP)
                * Matrix4::new_nonuniform_scaling(&vector![size, size, 1.0]),
            tex_idx,
            brightness,
        }
    }

    fn sun_brightness(stage: Stage) -> f32 {
        match stage {
            Stage::Dawn { progress } => utils::lerp(1.0, Self::SUN_BRIGHTNESS, progress),
            Stage::Day => Self::SUN_BRIGHTNESS,
            Stage::Dusk { progress } => utils::lerp(Self::SUN_BRIGHTNESS, 1.0, progress),
            Stage::Night => 1.0,
        }
    }
}

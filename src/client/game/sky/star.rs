use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{Instance, InstanceBuffer, MemoryState},
            effect::PostProcessor,
            program::{Program, PushConstants},
            utils::read_wgsl,
        },
    },
    server::{
        ServerEvent,
        game::clock::{Stage, Time},
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, UnitQuaternion, Vector3, point, vector};
use rand::{
    Rng, SeedableRng,
    distr::{Distribution, Uniform},
    rngs::SmallRng,
};
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, PI};
use winit::event::WindowEvent;

pub struct StarDome {
    stars: Box<[Star]>,
    instance_buffer: InstanceBuffer<StarInstance>,
    program: Program,
    pc: StarPushConstants,
    updated_rotation: Option<UnitQuaternion<f32>>,
}

impl StarDome {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let count = CLIENT_CONFIG.sky.star.count;
        let stars = {
            let mut rng = SmallRng::seed_from_u64(8008);
            let generator = StarGenerator::default();
            (0..count).map(|_| generator.generate(&mut rng)).collect()
        };
        let instance_buffer = InstanceBuffer::new(renderer, MemoryState::Uninit(count));
        let program = Program::new(
            renderer,
            read_wgsl("assets/shaders/star.wgsl"),
            &[StarInstance::desc()],
            &[player_bind_group_layout],
            &[StarPushConstants::range()],
            None,
            None,
            PostProcessor::FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        Self {
            stars,
            instance_buffer,
            program,
            pc: Default::default(),
            updated_rotation: Some(Time::default().sky_rotation()),
        }
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, player_bind_group: &wgpu::BindGroup) {
        if self.pc.opacity != 0.0 {
            self.program.bind(render_pass, [player_bind_group]);
            self.pc.set(render_pass);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..6, 0..self.instance_buffer.len());
        }
    }

    fn instances(&self) -> Option<impl Iterator<Item = StarInstance>> {
        self.updated_rotation.map(|rotation| {
            self.stars
                .iter()
                .map(move |&star| StarInstance::new(star, rotation))
        })
    }
}

impl EventHandler for StarDome {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::ServerEvent(ServerEvent::TimeUpdated(time)) => {
                self.pc = StarPushConstants::new(time.stage());
                self.updated_rotation = Some(time.sky_rotation());
            }
            Event::WindowEvent(WindowEvent::RedrawRequested) => {
                if let Some(instances) = self.instances() {
                    self.instance_buffer
                        .write(renderer, &instances.collect::<Vec<_>>());
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
struct Star {
    coords: Point3<f32>,
    rotation: f32,
}

impl Star {
    fn new(theta: f32, phi: f32, rotation: f32) -> Self {
        Self {
            coords: point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()],
            rotation,
        }
    }
}

struct StarGenerator {
    theta: Uniform<f32>,
    cos_phi: Uniform<f32>,
    rotation: Uniform<f32>,
}

impl StarGenerator {
    fn generate<R: Rng>(&self, rng: &mut R) -> Star {
        Star::new(
            self.theta.sample(rng),
            self.cos_phi.sample(rng).acos(),
            self.rotation.sample(rng),
        )
    }
}

impl Default for StarGenerator {
    fn default() -> Self {
        Self {
            theta: Uniform::new_inclusive(-PI, PI).unwrap_or_else(|_| unreachable!()),
            cos_phi: Uniform::new_inclusive(-1.0, 1.0).unwrap_or_else(|_| unreachable!()),
            rotation: Uniform::new(0.0, FRAC_PI_2).unwrap_or_else(|_| unreachable!()),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarInstance {
    m: Matrix4<f32>,
}

impl StarInstance {
    fn new(Star { coords, rotation }: Star, sky_rotation: UnitQuaternion<f32>) -> Self {
        let size = CLIENT_CONFIG.sky.star.size;
        Self {
            m: Matrix4::face_towards(&(sky_rotation * coords), &Point3::origin(), &Vector3::y())
                * Matrix4::new_rotation(Vector3::z() * rotation)
                    .prepend_nonuniform_scaling(&vector![size, size, 1.0]),
        }
    }
}

impl Instance for StarInstance {
    const ATTRIBS: &[wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarPushConstants {
    opacity: f32,
}

impl StarPushConstants {
    fn new(stage: Stage) -> Self {
        let brightness = CLIENT_CONFIG.sky.star.brightness;
        Self {
            opacity: stage.lerp(-brightness / 2.0, brightness).max(0.0),
        }
    }
}

impl Default for StarPushConstants {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl PushConstants for StarPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::FRAGMENT;
}

#[derive(Deserialize)]
pub struct StarConfig {
    size: f32,
    brightness: f32,
    count: usize,
}

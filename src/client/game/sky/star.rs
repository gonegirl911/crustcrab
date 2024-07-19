use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            buffer::{Instance, InstanceBuffer, MemoryState},
            effect::PostProcessor,
            program::{Program, PushConstants},
            Renderer,
        },
        CLIENT_CONFIG,
    },
    server::{
        game::clock::{Stage, Time},
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, vector, Matrix4, Point3, UnitQuaternion, Vector3};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, PI};
use winit::event::WindowEvent;

pub struct StarDome {
    stars: Vec<Star>,
    buffer: InstanceBuffer<StarInstance>,
    program: Program,
    pc: StarPushConstants,
    updated_rotation: Option<UnitQuaternion<f32>>,
}

impl StarDome {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let count = CLIENT_CONFIG.sky.star.count;
        let stars = {
            let mut rng = StdRng::seed_from_u64(6);
            (0..count).map(|_| Star::new(&mut rng)).collect()
        };
        let buffer = InstanceBuffer::new(renderer, MemoryState::Uninit(count));
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/star.wgsl"),
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
            buffer,
            program,
            pc: Default::default(),
            updated_rotation: Some(Time::default().sky_rotation()),
        }
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, player_bind_group: &wgpu::BindGroup) {
        if self.pc.opacity != 0.0 {
            self.program.bind(render_pass, [player_bind_group]);
            self.pc.set(render_pass);
            render_pass.set_vertex_buffer(0, self.buffer.slice(..));
            render_pass.draw(0..6, 0..self.buffer.len());
        }
    }

    fn instances(&self) -> Option<impl Iterator<Item = StarInstance> + '_> {
        self.updated_rotation.map(|rotation| {
            self.stars
                .iter()
                .copied()
                .map(move |star| StarInstance::new(star, rotation))
        })
    }
}

impl EventHandler for StarDome {
    type Context<'a> = &'a Renderer;

    fn handle(&mut self, event: &Event, renderer: Self::Context<'_>) {
        match event {
            Event::UserEvent(ServerEvent::TimeUpdated(time)) => {
                self.pc = StarPushConstants::new(time.stage());
                self.updated_rotation = Some(time.sky_rotation());
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                if let Some(instances) = self.instances() {
                    self.buffer.write(renderer, &instances.collect::<Vec<_>>());
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
    fn new<R: Rng>(rng: &mut R) -> Self {
        Self {
            coords: Self::spherical_coords(rng),
            rotation: rng.gen_range(0.0..FRAC_PI_2),
        }
    }

    fn spherical_coords<R: Rng>(rng: &mut R) -> Point3<f32> {
        let theta = rng.gen_range(-PI..=PI);
        let phi = rng.gen_range(-1.0f32..=1.0).acos();
        point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()]
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
    const ATTRIBS: &'static [wgpu::VertexAttribute] =
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

use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{MemoryState, VertexBuffer},
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            utils::{Immediates, Vertex, read_wgsl},
        },
    },
    server::{ServerEvent, game::clock::Time},
    shared::utils,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, point};
use rand::{
    Rng, SeedableRng,
    distr::{Distribution, Uniform},
    rngs::SmallRng,
};
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, PI};

pub struct StarDome {
    instance_buffer: VertexBuffer<StarInstance>,
    render_pipeline: RenderPipeline,
    imm: StarImmediates,
}

impl StarDome {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let instance_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("assets/shaders/star.wgsl"))
            .bind_group_layouts(&[player_bind_group_layout])
            .immediate_size(StarImmediates::SIZE)
            .buffers(&[StarInstance::desc()])
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState::ALPHA_BLENDING)
            .build();
        Self {
            instance_buffer,
            render_pipeline,
            imm: StarImmediates::new(Default::default()),
        }
    }

    pub fn draw(&self, render_pass: &mut wgpu::RenderPass, player_bind_group: &wgpu::BindGroup) {
        if self.imm.opacity != 0.0 {
            self.render_pipeline.bind(render_pass, [player_bind_group]);
            self.imm.set(render_pass);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            render_pass.draw(0..6, 0..self.instance_buffer.len());
        }
    }

    fn instances() -> impl Iterator<Item = StarInstance> {
        let mut rng = SmallRng::seed_from_u64(80085);
        let generator = StarGenerator::default();
        let count = CLIENT_CONFIG.sky.star.count;
        (0..count).map(move |_| generator.generate(&mut rng))
    }
}

impl EventHandler for StarDome {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::ServerEvent(ServerEvent::TimeUpdated(time)) = *event {
            self.imm = StarImmediates::new(time);
        }
    }
}

struct StarGenerator {
    theta: Uniform<f32>,
    cos_phi: Uniform<f32>,
    rotation: Uniform<f32>,
}

impl StarGenerator {
    fn generate<R: Rng>(&self, rng: &mut R) -> StarInstance {
        StarInstance::new(
            self.theta.sample(rng),
            self.cos_phi.sample(rng).acos(),
            self.rotation.sample(rng),
        )
    }
}

impl Default for StarGenerator {
    fn default() -> Self {
        Self {
            theta: Uniform::new_inclusive(-PI, PI).unwrap(),
            cos_phi: Uniform::new_inclusive(-1.0, 1.0).unwrap(),
            rotation: Uniform::new(0.0, FRAC_PI_2).unwrap(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarInstance {
    coords: Point3<f32>,
    rotation: f32,
}

impl StarInstance {
    fn new(theta: f32, phi: f32, rotation: f32) -> Self {
        Self {
            coords: point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()],
            rotation,
        }
    }
}

impl Vertex for StarInstance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBS: &[wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarImmediates {
    sky_rotation: Matrix4<f32>,
    size: f32,
    opacity: f32,
}

impl StarImmediates {
    fn new(time: Time) -> Self {
        let size = CLIENT_CONFIG.sky.star.size;
        let brightness = CLIENT_CONFIG.sky.star.brightness;
        let nightness = time.nightness();
        Self {
            sky_rotation: time.sky_rotation().to_homogeneous(),
            size,
            opacity: utils::lerp(-brightness / 2.0, brightness, nightness).max(0.0),
        }
    }
}

impl Immediates for StarImmediates {}

#[derive(Deserialize)]
pub struct StarConfig {
    size: f32,
    brightness: f32,
    count: usize,
}

use crate::{
    CONFIG,
    game::clock::RenderTime,
    renderer::{
        self, Immediates as _, Renderer, Vertex as _,
        buffer::{MemoryState, vertex::VertexBuffer},
        post_process::PostProcessor,
        render_pipeline::{RenderPipeline, read_wgsl},
    },
};
use base::math::interpolation::ramp;
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, point};
use rand::{
    Rng, SeedableRng,
    distr::{Distribution, Uniform},
    rngs::SmallRng,
};
use serde::Deserialize;
use std::f32::consts::{FRAC_PI_2, PI};

pub(super) struct Dome {
    instance_buffer: VertexBuffer<Instance>,
    render_pipeline: RenderPipeline,
}

impl Dome {
    pub(super) fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let instance_buffer = VertexBuffer::new(
            renderer,
            MemoryState::Immutable(&Self::instances().collect::<Vec<_>>()),
        );
        let render_pipeline = RenderPipeline::builder()
            .renderer(renderer)
            .shader_desc(read_wgsl("client/assets/shaders/star.wgsl"))
            .bind_group_layouts(&[Some(player_uniform_bind_group_layout)])
            .immediate_size(Immediates::SIZE)
            .buffers(&[Some(Instance::desc())])
            .format(PostProcessor::FORMAT)
            .blend(wgpu::BlendState::ALPHA_BLENDING)
            .build();
        Self {
            instance_buffer,
            render_pipeline,
        }
    }

    #[rustfmt::skip]
    pub(super) fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_uniform_bind_group: &wgpu::BindGroup,
        time: &RenderTime,
    ) {
        let imm = Immediates::new(time);
        if imm.opacity == 0.0 {
            return;
        }

        self.render_pipeline.bind(render_pass, [player_uniform_bind_group]);
        imm.set(render_pass);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..self.instance_buffer.len());
    }

    fn instances() -> impl Iterator<Item = Instance> {
        let mut rng = SmallRng::seed_from_u64(80085);
        let generator = Generator::default();
        let count = CONFIG.sky.star.count;
        (0..count).map(move |_| generator.generate(&mut rng))
    }
}

struct Generator {
    theta: Uniform<f32>,
    cos_phi: Uniform<f32>,
    rotation: Uniform<f32>,
}

impl Generator {
    fn generate<R: Rng>(&self, rng: &mut R) -> Instance {
        Instance::new(
            self.theta.sample(rng),
            self.cos_phi.sample(rng).acos(),
            self.rotation.sample(rng),
        )
    }
}

impl Default for Generator {
    fn default() -> Self {
        Self {
            theta: Uniform::new_inclusive(-PI, PI).unwrap(),
            cos_phi: Uniform::new_inclusive(-1.0, 1.0).unwrap(),
            rotation: Uniform::new(0.0, FRAC_PI_2).unwrap(),
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Instance {
    coords: Point3<f32>,
    rotation: f32,
}

impl Instance {
    fn new(theta: f32, phi: f32, rotation: f32) -> Self {
        Self {
            coords: point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()],
            rotation,
        }
    }
}

impl renderer::Vertex for Instance {
    const STEP_MODE: wgpu::VertexStepMode = wgpu::VertexStepMode::Instance;
    const ATTRIBS: &[wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Immediates {
    sky_rotation: Matrix4<f32>,
    size: f32,
    opacity: f32,
}

impl Immediates {
    fn new(time @ &RenderTime { nightness, .. }: &RenderTime) -> Self {
        let sky_rotation = time.sky_rotation();
        let opacity = CONFIG.sky.star.brightness * ramp(1.0 / 3.0, 1.0, nightness);
        Self {
            sky_rotation: sky_rotation.to_homogeneous(),
            size: CONFIG.sky.star.size,
            opacity,
        }
    }
}

impl renderer::Immediates for Immediates {}

#[derive(Deserialize)]
pub(super) struct Config {
    size: f32,
    brightness: f32,
    count: usize,
}

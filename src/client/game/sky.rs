use crate::{
    client::renderer::{
        DepthBuffer, IndexedMesh, PostProcessor, Program, Renderer, Uniform, Vertex,
    },
    color::{Float3, Rgb},
    server::game::world::block::{Corner, Side, CORNERS, SIDE_CORNER_DELTAS, SIDE_DELTAS},
};
use bytemuck::{Pod, Zeroable};
use enum_map::Enum;
use nalgebra::{point, Point3, Similarity3, UnitQuaternion, Vector3};
use rand::prelude::*;
use std::f32::consts::PI;

pub struct Sky {
    dome: StarDome,
    uniform: Uniform<SkyUniformData>,
}

impl Sky {
    const COLOR: Rgb<f32> = Rgb::new(0.00304, 0.00335, 0.0075);
    const LIGHT_INTENSITY: Rgb<f32> = Rgb::new(0.15, 0.15, 0.3);

    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        Self {
            dome: StarDome::new(renderer, player_bind_group_layout),
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

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
    ) {
        self.dome.draw(
            &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Self::COLOR.into()),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            }),
            player_bind_group,
        );
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

struct StarDome {
    mesh: IndexedMesh<StarVertex, u32>,
    program: Program,
}

impl StarDome {
    fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let distribution = StarDistribution::new(20000, Vector3::y());
        let mesh = IndexedMesh::new(
            renderer,
            &distribution.vertices().collect::<Vec<_>>(),
            &distribution.indices().collect::<Vec<_>>(),
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../assets/shaders/star.wgsl"),
            &[StarVertex::desc()],
            &[player_bind_group_layout],
            &[],
            PostProcessor::FORMAT,
            None,
            Some(wgpu::Face::Back),
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        Self { mesh, program }
    }

    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
    ) {
        self.program.bind(render_pass, [player_bind_group]);
        self.mesh.draw(render_pass);
    }
}

struct StarDistribution {
    count: usize,
    light_dir: Vector3<f32>,
}

impl StarDistribution {
    fn new(count: usize, light_dir: Vector3<f32>) -> Self {
        Self { count, light_dir }
    }

    fn vertices(&self) -> impl Iterator<Item = StarVertex> + '_ {
        let mut rng = rand::thread_rng();
        (0..self.count).flat_map(move |_| {
            let transform = Self::transform(&mut rng);
            SIDE_DELTAS.iter().flat_map(move |(side, delta)| {
                let normal = (transform * delta.cast()).normalize();
                let light_factor = self.light_factor(normal);
                SIDE_CORNER_DELTAS[side].values().map(move |delta| {
                    let coords = transform * Point3::from(delta.cast());
                    StarVertex::new(coords, light_factor)
                })
            })
        })
    }

    fn indices(&self) -> impl Iterator<Item = u32> {
        (0..self.count * Side::LENGTH).flat_map(|i| {
            let curr = i * Corner::LENGTH;
            CORNERS
                .into_iter()
                .map(move |corner| (curr + corner.into_usize()) as u32)
        })
    }

    fn light_factor(&self, normal: Vector3<f32>) -> f32 {
        0.2 + ((1.0 + normal.dot(&self.light_dir)) * 0.5) * 0.8
    }

    fn transform<R: Rng>(rng: &mut R) -> Similarity3<f32> {
        Similarity3::from_parts(
            Self::spherical_coords(rng).into(),
            UnitQuaternion::from_euler_angles(
                rng.gen_range(-PI..=PI),
                rng.gen_range(-PI..=PI),
                rng.gen_range(-PI..=PI),
            ),
            rng.gen_range(0.00075..=0.0075),
        )
    }

    fn spherical_coords<R: Rng>(rng: &mut R) -> Point3<f32> {
        let theta = rng.gen_range(-PI..=PI);
        let phi = rng.gen_range(-1.0f32..=1.0).acos();
        point![theta.cos() * phi.sin(), phi.cos(), theta.sin() * phi.sin()]
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct StarVertex {
    coords: Point3<f32>,
    light_factor: f32,
}

impl StarVertex {
    fn new(coords: Point3<f32>, light_factor: f32) -> Self {
        Self {
            coords,
            light_factor,
        }
    }
}

impl Vertex for StarVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
}

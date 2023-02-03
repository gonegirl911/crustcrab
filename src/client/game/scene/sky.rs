use super::depth_buffer::DepthBuffer;
use crate::client::renderer::{ImageTexture, IndexedMesh, Program, Renderer, Vertex};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::f32::consts::{PI, TAU};

pub struct Sky {
    mesh: IndexedMesh<SphereVertex, u16>,
    color_map: ImageTexture,
    program: Program,
}

impl Sky {
    pub fn new(
        renderer: &Renderer,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
        clock_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let sphere = Sphere::new(32, 16);
        let mesh = IndexedMesh::new(
            renderer,
            &sphere.vertices().collect::<Vec<_>>(),
            &sphere.indices().collect::<Vec<_>>(),
        );
        let color_map = ImageTexture::new(
            renderer,
            include_bytes!("../../../../assets/textures/sky.png"),
            false,
        );
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/sky.wgsl"),
            &[SphereVertex::desc()],
            &[
                camera_bind_group_layout,
                clock_bind_group_layout,
                color_map.bind_group_layout(),
            ],
            &[],
            None,
            Some(wgpu::Face::Front),
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        Self {
            mesh,
            color_map,
            program,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.color_map.bind_group_layout()
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.color_map.bind_group()
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
        clock_bind_group: &'a wgpu::BindGroup,
    ) {
        self.program.draw(
            render_pass,
            [
                camera_bind_group,
                clock_bind_group,
                self.color_map.bind_group(),
            ],
        );
        self.mesh.draw(render_pass);
    }
}

struct Sphere {
    sectors: u16,
    stacks: u16,
}

impl Sphere {
    fn new(sectors: u16, stacks: u16) -> Self {
        Self { sectors, stacks }
    }

    fn vertices(&self) -> impl Iterator<Item = SphereVertex> + '_ {
        (0..=self.stacks).flat_map(move |y| {
            let lat = PI * (0.5 - y as f32 / self.stacks as f32);
            (0..=self.sectors).map(move |x| {
                let long = TAU * x as f32 / self.sectors as f32;
                SphereVertex::new(
                    point![long.cos() * lat.cos(), lat.sin(), long.sin() * lat.cos()],
                    y as f32 / self.stacks as f32,
                )
            })
        })
    }

    fn indices(&self) -> impl Iterator<Item = u16> + '_ {
        (0..self.stacks).flat_map(move |y| {
            (0..self.sectors).flat_map(move |x| {
                let k1 = y * (self.sectors + 1) + x;
                let k2 = (y + 1) * (self.sectors + 1) + x;
                (y != 0)
                    .then_some([k1, k2, k1 + 1])
                    .into_iter()
                    .chain((y != self.stacks - 1).then_some([k1 + 1, k2, k2 + 1]))
                    .flatten()
            })
        })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct SphereVertex {
    coords: Point3<f32>,
    tex_v: f32,
}

impl SphereVertex {
    fn new(coords: Point3<f32>, tex_v: f32) -> Self {
        Self { coords, tex_v }
    }
}

impl Vertex for SphereVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32];
}

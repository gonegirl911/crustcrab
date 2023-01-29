use crate::client::renderer::{IndexedMesh, Renderer, Vertex};
use bytemuck::{Pod, Zeroable};
use nalgebra::Point3;

pub struct Crosshair {
    mesh: IndexedMesh<CrossVertex, u16>,
    render_pipeline: wgpu::RenderPipeline,
}

impl Crosshair {
    pub fn new(
        renderer @ Renderer { device, config, .. }: &Renderer,
        output_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let cross = Cross::new(18, 2);
        let mesh = IndexedMesh::new(
            renderer,
            &cross.vertices().collect::<Vec<_>>(),
            &cross.indices().collect::<Vec<_>>(),
        );
        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../../../assets/shaders/crosshair.wgsl"
        ));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[output_bind_group_layout],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[CrossVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
        });
        Self {
            mesh,
            render_pipeline,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        output_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, output_bind_group, &[]);
        self.mesh.draw(render_pass);
    }
}

pub struct Cross {
    size: u32,
    thickness: u32,
}

impl Cross {
    pub fn new(size: u32, thickness: u32) -> Self {
        Self { size, thickness }
    }

    pub fn vertices(&self) -> impl Iterator<Item = CrossVertex> {
        std::iter::empty()
    }

    pub fn indices(&self) -> impl Iterator<Item = u16> {
        std::iter::empty()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct CrossVertex {
    coords: Point3<f32>,
}

impl CrossVertex {
    fn new(coords: Point3<f32>) -> Self {
        Self { coords }
    }
}

impl Vertex for CrossVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
}

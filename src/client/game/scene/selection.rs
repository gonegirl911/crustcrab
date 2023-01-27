use crate::{
    client::{
        event_loop::{Event, EventHandler},
        game::scene::depth_buffer::DepthBuffer,
        renderer::Renderer,
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::mem;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

pub struct BlockSelection {
    mesh: BlockSelectionMesh,
    coords: Option<Point3<i32>>,
    render_pipeline: wgpu::RenderPipeline,
}

impl BlockSelection {
    pub fn new(
        renderer @ Renderer { device, config, .. }: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let mesh =
            BlockSelectionMesh::new(renderer, &VERTICES.map(BlockSelectionVertex::new), &INDICES);
        let coords = None;

        let shader = device.create_shader_module(wgpu::include_wgsl!(
            "../../../../assets/shaders/selection.wgsl"
        ));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[player_bind_group_layout],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..mem::size_of::<BlockSelectionPushConstants>() as u32,
                }],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[BlockSelectionVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthBuffer::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
        });

        Self {
            mesh,
            coords,
            render_pipeline,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
    ) {
        if let Some(coords) = self.coords {
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, player_bind_group, &[]);
            self.mesh.draw(render_pass, coords);
        }
    }
}

impl EventHandler for BlockSelection {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        if let Event::UserEvent(ServerEvent::BlockSelected { coords }) = event {
            self.coords = *coords;
        }
    }
}

struct BlockSelectionMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl BlockSelectionMesh {
    fn new(
        Renderer { device, .. }: &Renderer,
        vertices: &[BlockSelectionVertex],
        indices: &[u16],
    ) -> Self {
        Self {
            vertex_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        }
    }

    fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, coords: Point3<i32>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&[BlockSelectionPushConstants::new(coords)]),
        );
        render_pass.draw_indexed(0..self.len(), 0, 0..1);
    }

    fn len(&self) -> u32 {
        (self.index_buffer.size() / mem::size_of::<u16>() as u64) as u32
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockSelectionVertex(u32);

impl BlockSelectionVertex {
    fn new(coords: Point3<u8>) -> Self {
        let mut data = 0;
        data |= coords.x as u32;
        data |= (coords.y << 1) as u32;
        data |= (coords.z << 2) as u32;
        Self(data)
    }

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Uint32],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockSelectionPushConstants {
    coords: Point3<f32>,
}

impl BlockSelectionPushConstants {
    fn new(coords: Point3<i32>) -> Self {
        Self {
            coords: coords.cast(),
        }
    }
}

const VERTICES: [Point3<u8>; 8] = [
    point![0, 0, 0],
    point![1, 0, 0],
    point![1, 1, 0],
    point![0, 1, 0],
    point![0, 0, 1],
    point![1, 0, 1],
    point![1, 1, 1],
    point![0, 1, 1],
];

#[rustfmt::skip]
const INDICES: [u16; 36] = [
    0, 1, 2, 0, 2, 3,
    1, 5, 6, 1, 6, 2,
    5, 4, 7, 5, 7, 6,
    4, 0, 3, 4, 3, 7,
    3, 2, 6, 3, 6, 7,
    4, 5, 1, 4, 1, 0,
];

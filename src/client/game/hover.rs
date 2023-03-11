use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{DepthBuffer, IndexedMesh, Program, Renderer, Vertex},
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::mem;

pub struct BlockHover {
    outline: BlockHighlight,
    coords: Option<Point3<i64>>,
}

impl BlockHover {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            outline: BlockHighlight::new(renderer, player_bind_group_layout, sky_bind_group_layout),
            coords: None,
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        depth_buffer_view: &wgpu::TextureView,
    ) {
        if let Some(coords) = self.coords {
            self.outline.draw(
                &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_buffer_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: false,
                        }),
                        stencil_ops: None,
                    }),
                }),
                player_bind_group,
                sky_bind_group,
                coords,
            );
        }
    }
}

impl EventHandler for BlockHover {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        if let Event::UserEvent(ServerEvent::BlockHovered { coords }) = event {
            self.coords = *coords;
        }
    }
}

struct BlockHighlight {
    mesh: IndexedMesh<BlockHighlightVertex, u16>,
    program: Program,
}

impl BlockHighlight {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            mesh: IndexedMesh::new(
                renderer,
                &VERTEX_COORDS.map(BlockHighlightVertex::new),
                &INDICES,
            ),
            program: Program::new(
                renderer,
                wgpu::include_wgsl!("../../../assets/shaders/highlight.wgsl"),
                &[BlockHighlightVertex::desc()],
                &[player_bind_group_layout, sky_bind_group_layout],
                &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..mem::size_of::<BlockHighlightPushConstants>() as u32,
                }],
                None,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Some(wgpu::Face::Back),
                Some(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
            ),
        }
    }

    #[rustfmt::skip]
    fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        sky_bind_group: &'a wgpu::BindGroup,
        coords: Point3<i64>,
    ) {
        self.program.bind(render_pass, [player_bind_group, sky_bind_group]);
        render_pass.set_push_constants(
            wgpu::ShaderStages::VERTEX,
            0,
            bytemuck::cast_slice(&[BlockHighlightPushConstants::new(coords)]),
        );
        self.mesh.draw(render_pass);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockHighlightVertex {
    coords: Point3<f32>,
}

impl BlockHighlightVertex {
    fn new(coords: Point3<f32>) -> Self {
        Self { coords }
    }
}

impl Vertex for BlockHighlightVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockHighlightPushConstants {
    coords: Point3<f32>,
}

impl BlockHighlightPushConstants {
    fn new(coords: Point3<i64>) -> Self {
        Self {
            coords: coords.cast(),
        }
    }
}

const VERTEX_COORDS: [Point3<f32>; 8] = [
    point![0.0, 0.0, 0.0],
    point![1.0, 0.0, 0.0],
    point![1.0, 1.0, 0.0],
    point![0.0, 1.0, 0.0],
    point![0.0, 0.0, 1.0],
    point![1.0, 0.0, 1.0],
    point![1.0, 1.0, 1.0],
    point![0.0, 1.0, 1.0],
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

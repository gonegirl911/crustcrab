use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{IndexBuffer, MemoryState, VertexBuffer},
            effect::PostProcessor,
            program::Program,
            texture::screen::DepthBuffer,
            utils::{Immediates, Vertex, read_wgsl},
        },
    },
    server::{
        ServerEvent,
        game::world::{BlockHoverData, block::BlockLight},
    },
    shared::bound::Aabb,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Vector3, vector};

pub struct BlockHover {
    highlight: BlockHighlight,
    data: Option<BlockHoverData>,
}

impl BlockHover {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            highlight: BlockHighlight::new(
                renderer,
                player_bind_group_layout,
                sky_bind_group_layout,
            ),
            data: None,
        }
    }

    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
    ) {
        if let Some(BlockHoverData { hitbox, brightness }) = self.data {
            self.highlight.draw(
                &mut encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                }),
                player_bind_group,
                sky_bind_group,
                &BlockHighlightImmediates::new(hitbox, brightness),
            );
        }
    }
}

impl EventHandler for BlockHover {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::ServerEvent(ServerEvent::BlockHovered(data)) = *event {
            self.data = data;
        }
    }
}

struct BlockHighlight {
    vertex_buffer: VertexBuffer<BlockHighlightVertex>,
    index_buffer: IndexBuffer<u16>,
    program: Program,
}

impl BlockHighlight {
    fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout,
        sky_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            vertex_buffer: VertexBuffer::new(
                renderer,
                MemoryState::Immutable(&DELTAS.map(BlockHighlightVertex::new)),
            ),
            index_buffer: IndexBuffer::new(renderer, MemoryState::Immutable(&INDICES)),
            program: Program::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/highlight.wgsl"))
                .bind_group_layouts(&[player_bind_group_layout, sky_bind_group_layout])
                .immediate_size(BlockHighlightImmediates::SIZE)
                .buffers(&[BlockHighlightVertex::desc()])
                .cull_mode(wgpu::Face::Back)
                .depth_stencil(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                })
                .format(PostProcessor::FORMAT)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
        }
    }

    #[rustfmt::skip]
    fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass,
        player_bind_group: &wgpu::BindGroup,
        sky_bind_group: &wgpu::BindGroup,
        imm: &BlockHighlightImmediates,
    ) {
        self.program.bind(render_pass, [player_bind_group, sky_bind_group]);
        imm.set(render_pass);
        self.vertex_buffer.draw_indexed(render_pass, &self.index_buffer);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockHighlightVertex {
    coords: Point3<f32>,
}

impl BlockHighlightVertex {
    fn new(delta: Vector3<f32>) -> Self {
        Self {
            coords: delta.into(),
        }
    }
}

impl Vertex for BlockHighlightVertex {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockHighlightImmediates {
    m: Matrix4<f32>,
    brightness: u32,
}

impl BlockHighlightImmediates {
    fn new(hitbox: Aabb, brightness: BlockLight) -> Self {
        Self {
            m: hitbox.pad(CLIENT_CONFIG.cloud.padding).to_homogeneous(),
            brightness: brightness.0,
        }
    }
}

impl Immediates for BlockHighlightImmediates {}

const DELTAS: [Vector3<f32>; 8] = [
    vector![0.0, 0.0, 0.0],
    vector![1.0, 0.0, 0.0],
    vector![1.0, 1.0, 0.0],
    vector![0.0, 1.0, 0.0],
    vector![0.0, 0.0, 1.0],
    vector![1.0, 0.0, 1.0],
    vector![1.0, 1.0, 1.0],
    vector![0.0, 1.0, 1.0],
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

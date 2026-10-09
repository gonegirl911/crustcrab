use crate::{
    client::{
        CLIENT_CONFIG,
        event_loop::{Event, EventHandler},
        renderer::{
            Renderer,
            buffer::{IndexBuffer, MemoryState, VertexBuffer},
            effect::PostProcessor,
            render_pipeline::RenderPipeline,
            texture::screen::DepthBuffer,
            utils::{Immediates, Vertex, read_wgsl},
        },
    },
    server::{
        ControlEvent,
        game::{block::BlockLight, world::BlockHoverData},
    },
    shared::bound::Aabb,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Vector3, vector};

pub struct BlockHighlight {
    vertex_buffer: VertexBuffer<BlockHighlightVertex>,
    index_buffer: IndexBuffer<u16>,
    render_pipeline: RenderPipeline,
    data: Option<BlockHighlightData>,
}

impl BlockHighlight {
    pub fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            vertex_buffer: VertexBuffer::new(
                renderer,
                MemoryState::Immutable(&DELTAS.map(BlockHighlightVertex::new)),
            ),
            index_buffer: IndexBuffer::new(renderer, MemoryState::Immutable(&INDICES)),
            render_pipeline: RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("assets/shaders/highlight.wgsl"))
                .bind_group_layouts(&[
                    player_uniform_bind_group_layout,
                    shading_texture_bind_group_layout,
                ])
                .immediate_size(BlockHighlightImmediates::SIZE)
                .buffers(&[BlockHighlightVertex::desc()])
                .cull_mode(wgpu::Face::Back)
                .depth_stencil(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                })
                .format(PostProcessor::FORMAT)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
            data: None,
        }
    }

    #[rustfmt::skip]
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_texture_bind_group: &wgpu::BindGroup,
        depth_view: &wgpu::TextureView,
        anchor: Point3<f64>,
    ) {
        let Some(BlockHighlightData { hitbox, brightness }) = self.data else {
            return;
        };

        let imm = BlockHighlightImmediates::new(hitbox, brightness, anchor);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
        });
        self.render_pipeline.bind(
            &mut render_pass,
            [player_uniform_bind_group, shading_texture_bind_group],
        );
        imm.set(&mut render_pass);
        self.vertex_buffer.draw_indexed(&mut render_pass, &self.index_buffer);
    }
}

impl EventHandler for BlockHighlight {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::ControlEvent(ControlEvent::BlockHovered(data)) = &event {
            self.data = data.as_deref().map(BlockHighlightData::new);
        }
    }
}

struct BlockHighlightData {
    hitbox: Aabb,
    brightness: BlockLight,
}

impl BlockHighlightData {
    fn new(data: &BlockHoverData) -> Self {
        Self {
            hitbox: data.hitbox(),
            brightness: data.brightness(),
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
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

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct BlockHighlightImmediates {
    m: Matrix4<f32>,
    brightness: u32,
}

impl BlockHighlightImmediates {
    fn new(hitbox: Aabb, brightness: BlockLight, anchor: Point3<f64>) -> Self {
        Self {
            m: hitbox
                .pad(CLIENT_CONFIG.cloud.padding as f64)
                .translate(-anchor.coords)
                .to_homogeneous()
                .cast(),
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

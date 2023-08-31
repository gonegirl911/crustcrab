use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{
            buffer::{IndexBuffer, MemoryState, Vertex, VertexBuffer},
            effect::PostProcessor,
            program::{Program, PushConstants},
            texture::screen::DepthBuffer,
            Renderer,
        },
    },
    server::{
        game::world::{block::BlockLight, BlockHoverData},
        ServerEvent,
    },
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{vector, Point3, Vector3};

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
        if let Some(BlockHoverData { coords, brightness }) = self.data {
            self.highlight.draw(
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
                        view: depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: false,
                        }),
                        stencil_ops: None,
                    }),
                }),
                player_bind_group,
                sky_bind_group,
                BlockHighlightPushConstants::new(coords, brightness),
            );
        }
    }
}

impl EventHandler for BlockHover {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, _: Self::Context<'_>) {
        if let Event::UserEvent(ServerEvent::BlockHovered(data)) = event {
            self.data = *data;
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
                MemoryState::Immutable(
                    &DELTAS.map(|delta| BlockHighlightVertex::new(delta.into())),
                ),
            ),
            index_buffer: IndexBuffer::new(renderer, MemoryState::Immutable(&INDICES)),
            program: Program::new(
                renderer,
                wgpu::include_wgsl!("../../../assets/shaders/highlight.wgsl"),
                &[BlockHighlightVertex::desc()],
                &[player_bind_group_layout, sky_bind_group_layout],
                &[BlockHighlightPushConstants::range()],
                PostProcessor::FORMAT,
                Some(wgpu::BlendState::ALPHA_BLENDING),
                Some(wgpu::Face::Back),
                Some(wgpu::DepthStencilState {
                    format: DepthBuffer::FORMAT,
                    depth_write_enabled: false,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: -4,
                        slope_scale: -0.05,
                        ..Default::default()
                    },
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
        pc: BlockHighlightPushConstants,
    ) {
        self.program.bind(render_pass, [player_bind_group, sky_bind_group]);
        pc.set(render_pass);
        self.vertex_buffer.draw_indexed(render_pass, &self.index_buffer);
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
    brightness: u32,
}

impl BlockHighlightPushConstants {
    fn new(coords: Point3<i64>, brightness: BlockLight) -> Self {
        Self {
            coords: coords.cast(),
            brightness: brightness.0,
        }
    }
}

impl PushConstants for BlockHighlightPushConstants {
    const STAGES: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX;
}

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

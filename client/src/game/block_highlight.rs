use super::player::RenderCamera;
use crate::{
    event::{self, Event},
    renderer::{
        self, Immediates as _, Renderer, Vertex as _,
        buffer::{MemoryState, index::IndexBuffer, vertex::VertexBuffer},
        post_process::PostProcessor,
        render_pipeline::{RenderPipeline, read_wgsl},
        texture::screen::depth::DepthTexture,
    },
};
use base::geometry::bounding::aabb::{Aabb3, LocalAabb3};
use bytemuck::{Pod, Zeroable};
use domain::{game::world::block, protocol::control};
use nalgebra::{Matrix4, Point3, point};

pub(super) struct BlockHighlight {
    vertex_buffer: VertexBuffer<Vertex>,
    index_buffer: IndexBuffer<u16>,
    render_pipeline: RenderPipeline,
    data: Option<Data>,
}

impl BlockHighlight {
    pub(super) fn new(
        renderer: &Renderer,
        player_uniform_bind_group_layout: &wgpu::BindGroupLayout,
        shading_texture_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        Self {
            vertex_buffer: VertexBuffer::new(
                renderer,
                MemoryState::Immutable(&POINTS.map(Vertex::new)),
            ),
            index_buffer: IndexBuffer::new(renderer, MemoryState::Immutable(&INDICES)),
            render_pipeline: RenderPipeline::builder()
                .renderer(renderer)
                .shader_desc(read_wgsl("client/assets/shaders/highlight.wgsl"))
                .bind_group_layouts(&[
                    Some(player_uniform_bind_group_layout),
                    Some(shading_texture_bind_group_layout),
                ])
                .immediate_size(Immediates::SIZE)
                .buffers(&[Some(Vertex::desc())])
                .cull_mode(wgpu::Face::Back)
                .depth_stencil(wgpu::DepthStencilState {
                    format: DepthTexture::FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: -1,
                        ..Default::default()
                    },
                })
                .format(PostProcessor::FORMAT)
                .blend(wgpu::BlendState::ALPHA_BLENDING)
                .build(),
            data: None,
        }
    }

    #[rustfmt::skip]
    pub(super) fn draw(
        &self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        player_uniform_bind_group: &wgpu::BindGroup,
        shading_texture_bind_group: &wgpu::BindGroup,
        depth_texture_view: &wgpu::TextureView,
        &RenderCamera { anchor, .. }: &RenderCamera,
    ) {
        let Some(Data { hitbox, brightness }) = self.data else {
            return;
        };

        let imm = Immediates::new(hitbox.relative_to(anchor), brightness);

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
                view: depth_texture_view,
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

impl event::Handler for BlockHighlight {
    type Context<'a> = ();

    fn handle(&mut self, event: &Event, (): Self::Context<'_>) {
        if let Event::Control(control::Event::BlockHovered(data)) = &event {
            self.data = data.as_deref().map(Data::new);
        }
    }
}

struct Data {
    hitbox: Aabb3,
    brightness: block::Light,
}

impl Data {
    fn new(data: &block::hover::Data) -> Self {
        Self {
            hitbox: data.hitbox,
            brightness: todo!(),
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Vertex {
    coords: Point3<f32>,
}

impl Vertex {
    fn new(coords: Point3<f32>) -> Self {
        Self { coords }
    }
}

impl renderer::Vertex for Vertex {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
struct Immediates {
    m: Matrix4<f32>,
    brightness: u32,
}

impl Immediates {
    fn new(hitbox: LocalAabb3, brightness: block::Light) -> Self {
        Self {
            m: hitbox.to_homogeneous(),
            brightness: brightness.0,
        }
    }
}

impl renderer::Immediates for Immediates {}

const POINTS: [Point3<f32>; 8] = [
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

use super::depth::DepthBuffer;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{IndexedMesh, Program, Renderer, Vertex},
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::{point, Point3};
use std::mem;

pub struct BlockHover {
    mesh: IndexedMesh<BlockHighlightVertex, u16>,
    coords: Option<Point3<i32>>,
    program: Program,
}

impl BlockHover {
    pub fn new(
        renderer: &Renderer,
        player_bind_group_layout: &wgpu::BindGroupLayout, 
        skylight_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let outline = BlockHighlight::new(0.001);
        let mesh = IndexedMesh::new(
            renderer,
            &outline.vertices().collect::<Vec<_>>(),
            &outline.indices().collect::<Vec<_>>(),
        );
        let coords = None;
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/highlight.wgsl"),
            &[BlockHighlightVertex::desc()],
            &[player_bind_group_layout, skylight_bind_group_layout],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..mem::size_of::<BlockHoverPushConstants>() as u32,
            }],
            Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(wgpu::Face::Back),
            Some(wgpu::DepthStencilState {
                format: DepthBuffer::FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
        );
        Self {
            mesh,
            coords,
            program,
        }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        player_bind_group: &'a wgpu::BindGroup,
        skylight_bind_group: &'a wgpu::BindGroup,
    ) {
        if let Some(coords) = self.coords {
            self.program.draw(render_pass, [player_bind_group, skylight_bind_group]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::VERTEX,
                0,
                bytemuck::cast_slice(&[BlockHoverPushConstants::new(coords)]),
            );
            self.mesh.draw(render_pass);
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

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockHoverPushConstants {
    coords: Point3<f32>,
}

impl BlockHoverPushConstants {
    fn new(coords: Point3<i32>) -> Self {
        Self {
            coords: coords.cast(),
        }
    }
}

struct BlockHighlight {
    padding: f32,
}

impl BlockHighlight {
    fn new(padding: f32) -> Self {
        Self { padding }
    }

    fn vertices(&self) -> impl Iterator<Item = BlockHighlightVertex> {
        let min = -self.padding;
        let max = 1.0 + self.padding;
        [
            point![min, min, min],
            point![max, min, min],
            point![max, max, min],
            point![min, max, min],
            point![min, min, max],
            point![max, min, max],
            point![max, max, max],
            point![min, max, max],
        ]
        .into_iter()
        .map(BlockHighlightVertex::new)
    }

    #[rustfmt::skip]
    fn indices(&self) -> impl Iterator<Item = u16> {    
        [
            0, 1, 2, 0, 2, 3,
            1, 5, 6, 1, 6, 2,
            5, 4, 7, 5, 7, 6,
            4, 0, 3, 4, 3, 7,
            3, 2, 6, 3, 6, 7,
            4, 5, 1, 4, 1, 0,
        ]
        .into_iter()
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

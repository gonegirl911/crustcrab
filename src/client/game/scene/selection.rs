use super::depth_buffer::DepthBuffer;
use crate::{
    client::{
        event_loop::{Event, EventHandler},
        renderer::{IndexedMesh, Program, Renderer, Vertex},
    },
    server::ServerEvent,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Point3;
use std::mem;

pub struct BlockSelection {
    mesh: IndexedMesh<BlockShellVertex, u16>,
    coords: Option<Point3<i32>>,
    program: Program,
}

impl BlockSelection {
    pub fn new(renderer: &Renderer, player_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let outline = BlockShell::new(0.001);
        let mesh = IndexedMesh::new(
            renderer,
            &outline.vertices().collect::<Vec<_>>(),
            &outline.indices().collect::<Vec<_>>(),
        );
        let coords = None;
        let program = Program::new(
            renderer,
            wgpu::include_wgsl!("../../../../assets/shaders/selection.wgsl"),
            &[BlockShellVertex::desc()],
            &[player_bind_group_layout],
            &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..mem::size_of::<BlockSelectionPushConstants>() as u32,
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
    ) {
        if let Some(coords) = self.coords {
            self.program.draw(render_pass, [player_bind_group]);
            render_pass.set_push_constants(
                wgpu::ShaderStages::VERTEX,
                0,
                bytemuck::cast_slice(&[BlockSelectionPushConstants::new(coords)]),
            );
            self.mesh.draw(render_pass);
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

struct BlockShell {
    padding: f32,
}

impl BlockShell {
    fn new(padding: f32) -> Self {
        Self { padding }
    }

    fn vertices(&self) -> impl Iterator<Item = BlockShellVertex> {
        std::iter::empty()
    }

    fn indices(&self) -> impl Iterator<Item = u16> {
        std::iter::empty()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct BlockShellVertex {
    coords: Point3<f32>,
}

impl BlockShellVertex {
    fn new(coords: Point3<f32>) -> Self {
        Self { coords }
    }
}

impl Vertex for BlockShellVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3];
}

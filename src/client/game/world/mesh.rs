use crate::{
    client::renderer::{
        Renderer,
        buffer::{MemoryState, VertexBuffer},
        utils::{BlendedMesh, Immediates, Vertex},
    },
    server::game::{
        block::{
            BlockLight,
            data::{RenderLayer, SideShade},
        },
        chunk::visibility::VisibilityGraph,
        coords,
    },
    shared::enum_map::EnumMap,
};
use bitfield::{BitRange, BitRangeMut};
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Point3, point};

pub struct ChunkMesh {
    pub opaque_part: Option<VertexBuffer<BlockVertex>>,
    pub cutout_part: Option<VertexBuffer<BlockVertex>>,
    pub blended_part: Option<BlendedMesh<Point3<f32>, BlockVertex>>,
    pub visibility_graph: VisibilityGraph,
}

impl ChunkMesh {
    pub fn new(
        renderer: &Renderer,
        vertices: EnumMap<RenderLayer, &[BlockVertex]>,
        visibility_graph: VisibilityGraph,
    ) -> Option<Self> {
        let opaque_part = VertexBuffer::try_new(
            renderer,
            MemoryState::Immutable(vertices[RenderLayer::Opaque]),
        );
        let cutout_part = VertexBuffer::try_new(
            renderer,
            MemoryState::Immutable(vertices[RenderLayer::Cutout]),
        );
        let blended_part = BlendedMesh::try_new(renderer, vertices[RenderLayer::Blended], |v| {
            let sum = v
                .iter()
                .fold(Point3::default(), |acc, v| acc + v.coords().coords)
                .cast();
            sum / v.len() as f32
        });
        if opaque_part.is_some()
            || cutout_part.is_some()
            || blended_part.is_some()
            || visibility_graph != VisibilityGraph::ALL_CONNECTED
        {
            Some(Self {
                opaque_part,
                cutout_part,
                blended_part,
                visibility_graph,
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BlockVertex {
    data: [u32; 2],
}

impl BlockVertex {
    pub fn new(
        coords: Point3<u8>,
        tex_index: u8,
        tex_coords: Point2<u8>,
        side_shade: SideShade,
        ao: u8,
        light: BlockLight,
    ) -> Self {
        let mut data = [0; 2];
        data[0].set_bit_range(4, 0, coords.x);
        data[0].set_bit_range(9, 5, coords.y);
        data[0].set_bit_range(14, 10, coords.z);
        data[0].set_bit_range(22, 15, tex_index);
        data[0].set_bit_range(31, 27, tex_coords.x);
        data[1].set_bit_range(31, 27, tex_coords.y);
        data[0].set_bit_range(24, 23, side_shade as u8);
        data[0].set_bit_range(26, 25, ao);
        data[1].set_bit_range(26, 0, light.0);
        Self { data }
    }

    fn coords(&self) -> Point3<u8> {
        point![
            self.data[0].bit_range(4, 0),
            self.data[0].bit_range(9, 5),
            self.data[0].bit_range(14, 10),
        ]
    }

    pub fn light(&self) -> BlockLight {
        BlockLight(self.data[1])
    }
}

impl Vertex for BlockVertex {
    const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32x2];
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BlockImmediates {
    chunk_coords: Point3<f32>,
}

impl BlockImmediates {
    pub fn new(chunk_coords: Point3<i32>, anchor: Point3<f64>) -> Self {
        let anchor = coords::chunk(anchor);
        let chunk_coords = chunk_coords - anchor.coords;
        Self {
            chunk_coords: chunk_coords.cast(),
        }
    }
}

impl Immediates for BlockImmediates {}

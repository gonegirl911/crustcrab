use crate::client::renderer::Vertex;
use bytemuck::{Pod, Zeroable};
use nalgebra::{Point2, Point3};

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct BlockVertex(u32);

impl BlockVertex {
    pub fn new(
        coords: Point3<u8>,
        tex_coords: Point2<u8>,
        atlas_coords: Point2<u8>,
        ambient_occlusion: u8,
    ) -> Self {
        let mut data = 0;
        data |= coords.x as u32;
        data |= (coords.y as u32) << 5;
        data |= (coords.z as u32) << 10;
        data |= (tex_coords.x as u32) << 15;
        data |= (tex_coords.y as u32) << 16;
        data |= (atlas_coords.x as u32) << 17;
        data |= (atlas_coords.y as u32) << 21;
        data |= (ambient_occlusion as u32) << 25;
        Self(data)
    }
}

impl Vertex for BlockVertex {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Uint32];
}

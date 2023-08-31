use super::world::BlockVertex;
use crate::client::renderer::{
    buffer::{Instance, InstanceBuffer, VertexBuffer},
    program::Program,
    texture::image::ImageTexture,
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Vector2;

pub struct CloudLayer {
    vertex_buffer: VertexBuffer<BlockVertex>,
    instance_buffer: InstanceBuffer<CloudInstance>,
    texture: ImageTexture,
    program: Program,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudInstance {
    offset: Vector2<f32>,
}

impl Instance for CloudInstance {
    const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x2];
}

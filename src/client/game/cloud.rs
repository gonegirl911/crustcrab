use super::world::BlockVertex;
use crate::client::renderer::{
    mesh::InstancedMesh,
    program::Program,
    texture::{image::ImageTexture, screen::InputOutputTexture},
};
use bytemuck::{Pod, Zeroable};
use nalgebra::Vector2;

pub struct CloudLayer {
    mesh: InstancedMesh<BlockVertex, CloudInstance>,
    texture: ImageTexture,
    program: Program,
    helper: InputOutputTexture,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudInstance {
    offset: Vector2<i32>,
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
struct CloudPushConstants {
    offset: Vector2<f32>,
}

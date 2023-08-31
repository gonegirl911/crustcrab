use super::Chunk;
use crate::server::game::world::{block::Block, World};
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkGenerator;

impl ChunkGenerator {
    pub fn gen(&self, coords: Point3<i32>) -> Chunk {
        if (World::Y_RANGE.start..4).contains(&coords.y) {
            Chunk::from_fn(|_| Block::Sand)
        } else {
            Default::default()
        }
    }
}

use super::Chunk;
use crate::server::game::world::block::Block;
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkGenerator;

impl ChunkGenerator {
    pub fn gen(&self, coords: Point3<i32>) -> Chunk {
        if coords.y == -1 {
            Chunk::from_fn(|_| Block::Sand)
        } else {
            Default::default()
        }
    }
}
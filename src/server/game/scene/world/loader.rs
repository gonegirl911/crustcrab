use super::{block::Block, chunk::Chunk};
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkLoader;

impl ChunkLoader {
    pub fn get(&self, coords: Point3<i32>) -> Chunk {
        if coords.y == 0 {
            Chunk::from_fn(|_| Block::Sand)
        } else {
            Default::default()
        }
    }
}

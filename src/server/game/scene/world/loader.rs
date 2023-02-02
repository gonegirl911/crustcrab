use super::{block::Block, chunk::Chunk};
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkLoader;

impl ChunkLoader {
    pub fn get(&self, coords: Point3<i32>) -> Chunk {
        let fill = if coords.y == 0 {
            Block::Grass
        } else {
            Block::Air
        };
        Chunk::from_fn(|_| fill)
    }
}

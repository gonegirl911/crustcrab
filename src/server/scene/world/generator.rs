use super::{block::Block, chunk::Chunk};
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkGenerator;

impl ChunkGenerator {
    pub fn get(&self, coords: Point3<i32>) -> Option<Chunk> {
        let fill = if coords.y == 0 {
            Block::Grass
        } else {
            Block::Air
        };
        Chunk::from_fn(|_| fill)
    }
}

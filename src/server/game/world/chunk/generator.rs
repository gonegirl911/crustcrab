use std::ops::Range;

use super::Chunk;
use crate::server::game::world::block::Block;
use nalgebra::Point3;

#[derive(Default)]
pub struct ChunkGenerator;

impl ChunkGenerator {
    pub const Y_RANGE: Range<i32> = -1..0;

    pub fn gen(&self, coords: Point3<i32>) -> Chunk {
        if Self::Y_RANGE.contains(&coords.y) {
            Chunk::from_fn(|_| Block::Sand)
        } else {
            Default::default()
        }
    }
}

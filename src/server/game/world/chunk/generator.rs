use super::Chunk;
use crate::{
    server::game::world::{World, block::Block},
    shared::utils,
};
use nalgebra::Point3;
use noise::{NoiseFn, Simplex};

#[derive(Default)]
pub struct ChunkGenerator {
    noise: Simplex,
}

impl ChunkGenerator {
    pub fn generate(&self, coords: Point3<i32>) -> Chunk {
        if !(World::Y_RANGE.start..4).contains(&coords.y) {
            return Default::default();
        }

        Chunk::from_fn(|block_coords| {
            let coords = utils::coords(coords, block_coords).cast() / Chunk::DIM as f64;
            if self.noise.get(coords.into()) > 0.0 {
                Block::SAND
            } else {
                Block::AIR
            }
        })
    }
}

use super::Chunk;
use crate::{
    server::game::world::{block::Block, World},
    shared::utils,
};
use nalgebra::Point3;
use noise::{NoiseFn, Simplex};

#[derive(Default)]
pub struct ChunkGenerator(Simplex);

impl ChunkGenerator {
    pub fn generate(&self, coords: Point3<i32>) -> Chunk {
        if (World::Y_RANGE.start..4).contains(&coords.y) {
            Chunk::from_fn(|block_coords| {
                let coords = utils::coords((coords, block_coords)).cast() / Chunk::DIM as f64;
                if self.0.get(coords.into()) > 0.0 {
                    Block::Sand
                } else {
                    Block::Air
                }
            })
        } else {
            Default::default()
        }
    }
}

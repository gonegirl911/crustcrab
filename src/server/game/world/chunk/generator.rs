use super::Chunk;
use crate::server::game::world::{block::Block, World};
use nalgebra::Point3;

#[derive(Clone, Copy, Default)]
pub struct ChunkGenerator;

impl ChunkGenerator {
    pub fn gen(self, coords: Point3<i32>) -> Chunk {
        if (World::Y_RANGE.start..4).contains(&coords.y) {
            Chunk::from_fn(|coords| {
                if (6..9).contains(&coords.x) && coords.y == 7 && (6..9).contains(&coords.z) {
                    Block::Glowstone
                } else if (4..11).contains(&coords.x)
                    && coords.y == 6
                    && (4..11).contains(&coords.z)
                {
                    Block::Sand
                } else {
                    Block::GlassCyan
                }
            })
        } else {
            Default::default()
        }
    }
}

use super::chunk::Chunk;
use nalgebra::Point;

pub fn from_parts<const D: usize>(chunk: Point<i32, D>, block: Point<u8, D>) -> Point<i64, D> {
    chunk.cast() * Chunk::DIM as i64 + block.cast().coords
}

pub fn chunk<const D: usize>(coords: Point<i64, D>) -> Point<i32, D> {
    coords.map(|c| c.div_floor(Chunk::DIM as i64) as i32)
}

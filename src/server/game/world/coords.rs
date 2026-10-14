use super::chunk::Chunk;
use nalgebra::{Point, Scalar};

pub fn chunk<W: WorldCoords>(coords: W) -> W::Point<i32> {
    coords.chunk_coords()
}

pub fn block<W: WorldCoords>(coords: W) -> W::Point<u8> {
    coords.block_coords()
}

pub impl(self) trait WorldCoords {
    type Point<T: Scalar>;

    fn chunk_coords(&self) -> Self::Point<i32>;

    fn block_coords(&self) -> Self::Point<u8>;
}

impl<const D: usize> WorldCoords for Point<i64, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn chunk_coords(&self) -> Self::Point<i32> {
        self.map(|c| c.div_floor(Chunk::DIM as i64) as i32)
    }

    fn block_coords(&self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }
}

impl<const D: usize> WorldCoords for Point<f64, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn chunk_coords(&self) -> Self::Point<i32> {
        self.map(|c| (c / Chunk::DIM as f64).floor() as i32)
    }

    fn block_coords(&self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as f64) as u8)
    }
}

pub fn from_parts<const D: usize>(chunk: Point<i32, D>, block: Point<u8, D>) -> Point<i64, D> {
    chunk.cast() * Chunk::DIM as i64 + block.cast().coords
}

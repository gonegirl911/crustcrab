use super::{Chunk, ChunkArea};
use crate::server::game::world::block::light::{BlockAreaLight, BlockLight};
use nalgebra::{Point3, Vector3};
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for ChunkLight {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

#[derive(Default)]
pub struct ChunkAreaLight([[[BlockLight; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl ChunkAreaLight {
    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        let coords = coords.coords.cast();
        BlockAreaLight::from_fn(|delta| self[coords + delta])
    }
}

impl Index<Vector3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = unsafe { ChunkArea::index_unchecked(delta) };
        &self.0[idx.x][idx.y][idx.z]
    }
}

impl IndexMut<Vector3<i8>> for ChunkAreaLight {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let idx = unsafe { ChunkArea::index_unchecked(delta) };
        &mut self.0[idx.x][idx.y][idx.z]
    }
}

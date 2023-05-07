use super::{Chunk, ChunkArea};
use crate::server::game::world::block::light::{BlockAreaLight, BlockLight};
use nalgebra::{Point3, Vector3};
use std::{
    mem,
    ops::{Index, IndexMut},
};

#[repr(align(16))]
#[derive(Default)]
pub struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl ChunkLight {
    pub fn is_empty(&self) -> bool {
        let expected = unsafe { mem::transmute([BlockLight::default(); 4]) };
        self.0
            .iter()
            .flatten()
            .flat_map(|lights| lights.chunks_exact(4))
            .all(|lights| *unsafe { &*lights.as_ptr().cast::<u128>() } == expected)
    }
}

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

    unsafe fn index_unchecked(delta: Vector3<i8>) -> Point3<usize> {
        delta
            .map(|c| (c + ChunkArea::PADDING as i8) as usize)
            .into()
    }
}

impl Index<Vector3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &self.0[idx.x][idx.y][idx.z]
    }
}

impl IndexMut<Vector3<i8>> for ChunkAreaLight {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &mut self.0[idx.x][idx.y][idx.z]
    }
}

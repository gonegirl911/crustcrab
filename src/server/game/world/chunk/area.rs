use super::Chunk;
use crate::{
    server::game::world::block::{
        area::{BlockArea, BlockAreaLight},
        Block, BlockLight,
    },
    shared::utils,
};
use nalgebra::{point, vector, Point3, Vector3};
use std::ops::{Index, IndexMut, Range};

#[derive(Default)]
pub struct ChunkArea([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl ChunkArea {
    const DIM: usize = Chunk::DIM + BlockArea::PADDING * 2;
    const PADDING: usize = BlockArea::PADDING.div_ceil(Chunk::DIM);
    const AXIS_RANGE: Range<i32> = -(Self::PADDING as i32)..1 + Self::PADDING as i32;
    const REM: usize = BlockArea::PADDING % Chunk::DIM;

    pub fn block_area(&self, coords: Point3<u8>) -> BlockArea {
        BlockArea::from_fn(|delta| self[coords.coords.cast() + delta])
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    pub fn block_deltas(delta: Vector3<i32>) -> impl Iterator<Item = (Point3<u8>, Vector3<i8>)> {
        let [dx, dy, dz] = delta.into();
        Self::block_axis_range(dx).flat_map(move |x| {
            Self::block_axis_range(dy).flat_map(move |y| {
                Self::block_axis_range(dz).map(move |z| {
                    (
                        point![x, y, z],
                        utils::coords((vector![dx, dy, dz], vector![x, y, z])).cast(),
                    )
                })
            })
        })
    }

    fn block_axis_range(dc: i32) -> Range<u8> {
        if dc == Self::AXIS_RANGE.start {
            (Chunk::DIM - Self::REM) as u8..Chunk::DIM as u8
        } else if dc == Self::AXIS_RANGE.end - 1 {
            0..Self::REM as u8
        } else {
            0..Chunk::DIM as u8
        }
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl Index<Vector3<i8>> for ChunkArea {
    type Output = Block;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

impl IndexMut<Vector3<i8>> for ChunkArea {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &mut self.0[x][y][z]
    }
}

#[derive(Default)]
pub struct ChunkAreaLight([[[BlockLight; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl ChunkAreaLight {
    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        BlockAreaLight::from_fn(|delta| self[coords.coords.cast() + delta])
    }
}

impl Index<Vector3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = ChunkArea::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

impl IndexMut<Vector3<i8>> for ChunkAreaLight {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let [x, y, z] = ChunkArea::index_unchecked(delta);
        &mut self.0[x][y][z]
    }
}

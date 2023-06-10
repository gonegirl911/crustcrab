pub mod area;
pub mod generator;

use super::{
    action::BlockAction,
    block::{Block, BlockLight},
};
use crate::shared::{
    bound::{Aabb, BoundingSphere},
    utils,
};
use nalgebra::{point, Point3, Vector3};
use std::{
    array, mem,
    ops::{Index, IndexMut},
};

#[repr(align(16))]
#[derive(Clone, Default)]
pub struct Chunk([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl Chunk {
    pub const DIM: usize = 16;

    fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].cast())))
        }))
    }

    pub fn is_empty(&self) -> bool {
        let expected = unsafe { mem::transmute([Block::Air; Self::DIM]) };
        self.0
            .iter()
            .flatten()
            .all(|blocks| *unsafe { mem::transmute::<_, &u128>(blocks) } == expected)
    }

    pub fn apply(&mut self, coords: Point3<u8>, action: &BlockAction) -> bool {
        self[coords].apply(action)
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: &BlockAction) {
        self[coords].apply_unchecked(action)
    }

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        (0..Self::DIM).flat_map(|x| {
            (0..Self::DIM).flat_map(move |y| (0..Self::DIM).map(move |z| point![x, y, z].cast()))
        })
    }

    fn bounding_box(coords: Point3<i32>) -> Aabb {
        Aabb::new(
            utils::coords((coords, Default::default())).cast(),
            Vector3::from_element(Self::DIM).cast(),
        )
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        Self::bounding_box(coords).into()
    }
}

impl Index<Point3<u8>> for Chunk {
    type Output = Block;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for Chunk {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

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

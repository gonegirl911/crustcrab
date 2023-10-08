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

#[derive(Default)]
pub struct Chunk {
    blocks: DataStore<Block>,
    non_air_count: u16,
}

impl Chunk {
    pub const DIM: usize = 16;

    fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        let mut non_air_count = 0;
        Self {
            blocks: DataStore::from_fn(|coords| {
                let block = f(coords);
                non_air_count += (block != Block::Air) as u16;
                block
            }),
            non_air_count,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.non_air_count == 0
    }

    pub fn apply(&mut self, coords: Point3<u8>, action: &BlockAction) -> bool {
        let prev = &mut self.blocks[coords];
        match action {
            BlockAction::Place(Block::Air) => false,
            BlockAction::Place(curr) if *prev == Block::Air => {
                *prev = *curr;
                self.non_air_count += 1;
                true
            }
            BlockAction::Destroy if *prev != Block::Air => {
                *prev = Block::Air;
                self.non_air_count -= 1;
                true
            }
            _ => false,
        }
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: &BlockAction) {
        let prev = &mut self.blocks[coords];
        let curr = match action {
            BlockAction::Place(block) => *block,
            BlockAction::Destroy => Block::Air,
        };
        match (mem::replace(prev, curr), curr) {
            (Block::Air, Block::Air) => {}
            (Block::Air, _) => self.non_air_count += 1,
            (_, Block::Air) => self.non_air_count -= 1,
            _ => {}
        }
    }

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        (0..Self::DIM).flat_map(|x| {
            (0..Self::DIM).flat_map(move |y| (0..Self::DIM).map(move |z| point![x, y, z].cast()))
        })
    }

    fn bounding_box(coords: Point3<i32>) -> Aabb {
        Aabb::new(
            utils::coords((coords, Default::default())).cast(),
            Vector3::repeat(Self::DIM).cast(),
        )
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        Self::bounding_box(coords).into()
    }
}

impl Index<Point3<u8>> for Chunk {
    type Output = Block;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.blocks[coords]
    }
}

#[derive(Default)]
pub struct ChunkLight {
    lights: DataStore<BlockLight>,
    non_zero_count: u16,
}

impl ChunkLight {
    pub fn set(&mut self, coords: Point3<u8>, value: BlockLight) -> bool {
        let prev = mem::replace(&mut self.lights[coords], value);
        if prev == value {
            false
        } else {
            if prev == Default::default() {
                self.non_zero_count += 1;
            } else if value == Default::default() {
                self.non_zero_count -= 1;
            }
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        self.non_zero_count == 0
    }
}

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.lights[coords]
    }
}

#[derive(Default)]
struct DataStore<T>([[[T; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl<T> DataStore<T> {
    fn from_fn<F: FnMut(Point3<u8>) -> T>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].cast())))
        }))
    }
}

impl<T> Index<Point3<u8>> for DataStore<T> {
    type Output = T;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl<T> IndexMut<Point3<u8>> for DataStore<T> {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

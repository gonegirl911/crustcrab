pub mod area;
pub mod generator;
pub mod visibility;

use super::{
    action::BlockAction,
    block::{Block, BlockLight},
};
use crate::shared::{
    bound::{Aabb, BoundingSphere},
    utils,
};
use bitvec::{BitArr, bitarr};
use nalgebra::{Point3, Vector3, point};
use std::{
    array, mem,
    ops::{Index, IndexMut},
};
use visibility::VisibilityGraph;

pub struct Chunk {
    blocks: ChunkDataStore<Block>,
    non_air_count: u16,
    glowing_count: u16,
    pub mut(self) visibility_graph: VisibilityGraph,
}

impl Chunk {
    pub const DIM: usize = 16;

    fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        let mut non_air_count = 0;
        let mut glowing_count = 0;
        Self {
            blocks: ChunkDataStore::from_fn(|coords| {
                let block = f(coords);
                non_air_count += (block != Block::AIR) as u16;
                glowing_count += block.data().is_glowing() as u16;
                block
            }),
            non_air_count,
            glowing_count,
            visibility_graph: VisibilityGraph::ALL_CONNECTED,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.non_air_count == 0
    }

    pub fn is_glowing(&self) -> bool {
        self.glowing_count != 0
    }

    pub fn apply(&mut self, coords: Point3<u8>, action: BlockAction) -> bool {
        let block = &mut self.blocks[coords];
        let prev = *block;
        if block.apply(action) {
            let curr = *block;
            self.adjust_counts(prev, curr);
            true
        } else {
            false
        }
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: BlockAction) {
        let block = &mut self.blocks[coords];
        let prev = *block;
        block.apply_unchecked(action);
        let curr = *block;
        self.adjust_counts(prev, curr);
    }

    pub fn recompute_visibility_graph(&mut self) {
        let opaque_set = ChunkBitSet::from_fn(|coords| self[coords].data().is_opaque());
        self.visibility_graph = VisibilityGraph::compute(opaque_set);
    }

    pub fn as_slice(&self) -> &[Block] {
        self.blocks.as_slice()
    }

    fn adjust_counts(&mut self, prev: Block, curr: Block) {
        self.non_air_count -= (prev != Block::AIR) as u16;
        self.non_air_count += (curr != Block::AIR) as u16;
        self.glowing_count -= prev.data().is_glowing() as u16;
        self.glowing_count += curr.data().is_glowing() as u16;
    }

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        (0..Self::DIM.pow(3)).map(|i| {
            let x = i / Self::DIM.pow(2);
            let y = i % Self::DIM.pow(2) / Self::DIM;
            let z = i % Self::DIM;
            point![x, y, z].cast()
        })
    }

    fn is_in_bounds(coords: Point3<i8>) -> Option<Point3<u8>> {
        const { assert!(Self::DIM <= i8::MAX as usize) };
        coords
            .iter()
            .all(|c| (0..Self::DIM as i8).contains(c))
            .then(|| coords.map(|c| c as u8))
    }

    fn bounding_box(coords: Point3<i32>) -> Aabb {
        Aabb::new(
            utils::coords(coords, Default::default()).cast(),
            Vector3::repeat(Self::DIM).cast(),
        )
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        Self::bounding_box(coords).into()
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            blocks: Default::default(),
            non_air_count: 0,
            glowing_count: 0,
            visibility_graph: VisibilityGraph::ALL_CONNECTED,
        }
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
    lights: ChunkDataStore<BlockLight>,
    non_zero_count: u16,
}

impl ChunkLight {
    pub fn placeholder() -> Self {
        Self {
            lights: ChunkDataStore::from_fn(|_| BlockLight::placeholder()),
            non_zero_count: Chunk::DIM.pow(3) as u16,
        }
    }

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

    pub fn set_unchecked(&mut self, coords: Point3<u8>, value: BlockLight) {
        let prev = mem::replace(&mut self.lights[coords], value);
        if prev == Default::default() {
            self.non_zero_count += 1;
        } else if value == Default::default() {
            self.non_zero_count -= 1;
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
pub struct ChunkDataStore<T>([[[T; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl<T> ChunkDataStore<T> {
    pub fn from_fn<F: FnMut(Point3<u8>) -> T>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].cast())))
        }))
    }

    fn as_slice(&self) -> &[T] {
        self.0.as_flattened().as_flattened()
    }
}

impl<T> Index<Point3<u8>> for ChunkDataStore<T> {
    type Output = T;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl<T> IndexMut<Point3<u8>> for ChunkDataStore<T> {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

pub struct ChunkBitSet(BitArr!(for Chunk::DIM.pow(3)));

impl ChunkBitSet {
    fn from_fn<F: FnMut(Point3<u8>) -> bool>(mut f: F) -> Self {
        let mut data = Self::default();
        for coords in Chunk::points() {
            data.set(coords, f(coords));
        }
        data
    }

    fn set(&mut self, coords: Point3<u8>, value: bool) {
        self.0.set(Self::index_unchecked(coords), value);
    }

    fn replace(&mut self, coords: Point3<u8>, value: bool) -> bool {
        self.0.replace(Self::index_unchecked(coords), value)
    }

    fn index_unchecked(coords: Point3<u8>) -> usize {
        let coords = coords.cast::<usize>();
        coords.x * Chunk::DIM.pow(2) + coords.y * Chunk::DIM + coords.z
    }
}

impl Default for ChunkBitSet {
    fn default() -> Self {
        Self(bitarr![0; Chunk::DIM.pow(3)])
    }
}

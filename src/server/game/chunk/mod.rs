pub mod area;
pub mod visibility;

use super::{
    block::{Block, BlockLight},
    coords,
    world::action::BlockAction,
};
use crate::shared::{
    bound::{Aabb, BoundingSphere},
    cuboid::Cuboid,
};
use bitvec::{BitArr, bitarr};
use nalgebra::{Point3, Vector3, point};
use std::{
    array,
    ops::{Index, IndexMut},
};
use visibility::VisibilityGraph;

#[derive(Clone)]
pub struct Chunk {
    blocks: ChunkDataStore<Block>,
    non_air_count: u16,
    pub mut(self) visibility_graph: VisibilityGraph,
}

impl Chunk {
    pub const DIM: usize = 16;
    pub const VOLUME: usize = Self::DIM.pow(3);

    pub fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        let mut non_air_count = 0;
        Self {
            blocks: ChunkDataStore::from_fn(|coords| {
                let block = f(coords);
                non_air_count += (block != Block::AIR) as u16;
                block
            }),
            non_air_count,
            visibility_graph: VisibilityGraph::ALL_CONNECTED,
        }
    }

    pub fn apply(&mut self, coords: Point3<u8>, action: BlockAction) -> bool {
        let block = &mut self.blocks[coords];
        let prev = *block;
        let is_valid = block.apply(action);
        let cur = *block;
        self.adjust_counts(prev, cur);
        is_valid
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: BlockAction) -> bool {
        let block = &mut self.blocks[coords];
        let prev = *block;
        block.apply_unchecked(action);
        let cur = *block;
        self.adjust_counts(prev, cur);
        prev != cur
    }

    pub fn is_empty(&self) -> bool {
        self.non_air_count == 0
    }

    pub fn recompute_visibility_graph(&mut self) {
        let opaque_set = ChunkBitSet::from_fn(|coords| self[coords].data().is_opaque());
        self.visibility_graph = VisibilityGraph::compute(opaque_set);
    }

    pub fn as_slice(&self) -> &[Block] {
        self.blocks.as_slice()
    }

    pub fn row(&self, coords: Point3<u8>, len: usize) -> &[Block] {
        self.blocks.row(coords, len)
    }

    fn adjust_counts(&mut self, prev: Block, cur: Block) {
        self.non_air_count -= (prev != Block::AIR) as u16;
        self.non_air_count += (cur != Block::AIR) as u16;
    }

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        Self::cuboid()
            .into_points()
            .map(|coords| coords.map(|c| c as u8))
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
            coords::from_parts(coords, Point3::origin()).cast(),
            Vector3::repeat(Self::DIM as f64),
        )
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        Self::bounding_box(coords).into()
    }

    fn cuboid() -> Cuboid {
        Cuboid::unit().scale(Self::DIM as i64)
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            blocks: Default::default(),
            non_air_count: 0,
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

pub struct ChunkLight(ChunkDataStore<BlockLight>);

impl ChunkLight {
    pub fn placeholder() -> Self {
        Self(ChunkDataStore::from_fn(|_| BlockLight::placeholder()))
    }

    pub fn row(&self, coords: Point3<u8>, len: usize) -> &[BlockLight] {
        self.0.row(coords, len)
    }
}

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords]
    }
}

#[derive(Clone, Default)]
struct ChunkDataStore<T>([[[T; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl<T> ChunkDataStore<T> {
    fn from_fn<F: FnMut(Point3<u8>) -> T>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].cast())))
        }))
    }

    fn as_slice(&self) -> &[T] {
        self.0.as_flattened().as_flattened()
    }

    fn row(&self, coords: Point3<u8>, len: usize) -> &[T] {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize..][..len]
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

pub struct ChunkBitSet(BitArr!(for Chunk::VOLUME));

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
        Chunk::cuboid().flatten(coords.cast()) as usize
    }
}

impl Default for ChunkBitSet {
    fn default() -> Self {
        Self(bitarr![0; Chunk::VOLUME])
    }
}

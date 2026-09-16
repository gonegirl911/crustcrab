pub mod area;
pub mod generator;
pub mod visibility;

use super::{
    action::BlockAction,
    block::{Block, BlockLight},
};
use crate::{
    server::game::world::block::area::BlockArea,
    shared::{
        bound::{Aabb, BoundingSphere},
        cuboid::Cuboid,
        utils,
    },
};
use area::ChunkArea;
use bitfield::Bit;
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

    pub fn apply(&mut self, coords: Point3<u8>, action: BlockAction) -> bool {
        let block = &mut self.blocks[coords];
        let prev = *block;
        let is_valid = block.apply(action);
        let cur = *block;
        self.adjust_counts(prev, cur);
        is_valid
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: BlockAction) {
        let block = &mut self.blocks[coords];
        let prev = *block;
        block.apply_unchecked(action);
        let cur = *block;
        self.adjust_counts(prev, cur);
    }

    pub fn is_empty(&self) -> bool {
        self.non_air_count == 0
    }

    pub fn recompute_visibility_graph(&mut self) {
        let opaque_set = ChunkBitSet::from_fn(|coords| self[coords].data().is_opaque());
        self.visibility_graph = VisibilityGraph::compute(opaque_set);
    }

    pub fn is_glowing(&self) -> bool {
        self.glowing_count != 0
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
        self.glowing_count -= prev.data().is_glowing() as u16;
        self.glowing_count += cur.data().is_glowing() as u16;
    }

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        Cuboid::unit()
            .scale(Self::DIM as i64)
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
            utils::coords(coords, Point3::origin()).cast(),
            Vector3::repeat(Self::DIM as f32),
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

#[derive(Clone, Default)]
pub struct ChunkLight {
    lights: ChunkDataStore<BlockLight>,
    non_zero_count: u16,
}

impl ChunkLight {
    pub fn from_fn<F: FnMut(Point3<u8>) -> BlockLight>(mut f: F) -> Self {
        let mut non_zero_count = 0;
        Self {
            lights: ChunkDataStore::from_fn(|coords| {
                let light = f(coords);
                non_zero_count += (light != Default::default()) as u16;
                light
            }),
            non_zero_count,
        }
    }

    pub fn placeholder() -> Self {
        Self {
            lights: ChunkDataStore::from_fn(|_| BlockLight::placeholder()),
            non_zero_count: Chunk::DIM.pow(3) as u16,
        }
    }

    pub fn set(&mut self, coords: Point3<u8>, value: BlockLight) -> bool {
        let prev = mem::replace(&mut self.lights[coords], value);
        let cur = value;
        self.adjust_count(prev, cur);
        prev == cur
    }

    pub fn is_empty(&self) -> bool {
        self.non_zero_count == 0
    }

    pub fn diff_reach(&self, other: &ChunkLight) -> Option<ChunkReach> {
        let mut reach = ChunkReach::default();
        for ((coords, a), b) in Chunk::points().zip(self.as_slice()).zip(other.as_slice()) {
            if a != b {
                reach.insert_block(coords);
            }
        }
        (!reach.is_empty()).then_some(reach)
    }

    fn as_slice(&self) -> &[BlockLight] {
        self.lights.as_slice()
    }

    pub fn row(&self, coords: Point3<u8>, len: usize) -> &[BlockLight] {
        self.lights.row(coords, len)
    }

    fn adjust_count(&mut self, prev: BlockLight, cur: BlockLight) {
        self.non_zero_count -= (prev != Default::default()) as u16;
        self.non_zero_count += (cur != Default::default()) as u16;
    }
}

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.lights[coords]
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

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct ChunkReach(u32);

impl ChunkReach {
    pub fn insert_block(&mut self, coords: Point3<u8>) {
        self.0 |= Self::block_reach(coords);
    }

    pub fn is_empty(self) -> bool {
        self == Default::default()
    }

    fn block_reach(coords: Point3<u8>) -> u32 {
        Self::axis_reach(coords.x, 9)
            * Self::axis_reach(coords.y, 3)
            * Self::axis_reach(coords.z, 1)
    }

    fn index(delta: Vector3<i32>) -> usize {
        const { assert!(ChunkArea::CHUNK_PADDING <= 1) };

        (9 * (delta.x + 1) + 3 * (delta.y + 1) + delta.z + 1) as usize
    }

    fn axis_reach(c: u8, stride: u32) -> u32 {
        let padding = BlockArea::PADDING as u8;
        let lo = (c < padding) as u32;
        let hi = (c >= Chunk::DIM as u8 - padding) as u32;
        lo | (1 << stride) | (hi << (stride * 2))
    }
}

impl IntoIterator for ChunkReach {
    type Item = Vector3<i32>;
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        ChunkArea::chunk_deltas().filter(move |&delta| self.0.bit(Self::index(delta)))
    }
}

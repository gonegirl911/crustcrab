use super::Chunk;
use crate::{
    server::game::block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaView, BlockLightAreaView},
    },
    shared::cuboid::Cuboid,
};
use bitfield::Bit;
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::ops::{Index, Range};

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkArea(ChunkAreaDataStore<Block>);

impl ChunkArea {
    const PADDING: usize = BlockArea::PADDING;
    const CHUNK_PADDING: usize = Self::PADDING.div_ceil(Chunk::DIM);
    const DIM: usize = Chunk::DIM + 2 * Self::PADDING;
    const CHUNK_DIM: usize = 1 + Self::CHUNK_PADDING * 2;
    const CHUNK_VOLUME: usize = Self::CHUNK_DIM.pow(3);

    pub fn block_area_view(&self, coords: Point3<u8>) -> BlockAreaView<'_> {
        BlockAreaView::new(&self.0, coords)
    }

    pub fn copy_row(&mut self, delta: Vector3<i8>, src: &[Block]) {
        self.0.copy_row(delta, src);
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i8>> {
        Self::chunk_cuboid()
            .into_points()
            .map(|coords| coords.coords.cast())
    }

    pub fn axis_range(dc: i8) -> Range<u8> {
        let dc = dc as i32;
        let chunk_dim = Chunk::DIM as i32;
        let padding = Self::PADDING as i32;
        let start = (-padding - dc * chunk_dim).max(0);
        let end = (chunk_dim + padding - dc * chunk_dim).min(chunk_dim);
        start as u8..end as u8
    }

    fn chunk_cuboid() -> Cuboid {
        Cuboid::unit().pad(Self::PADDING as i64)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkLightArea(ChunkAreaDataStore<BlockLight>);

impl ChunkLightArea {
    pub fn block_light_area_view(&self, coords: Point3<u8>) -> BlockLightAreaView<'_> {
        BlockLightAreaView::new(&self.0, coords)
    }

    pub fn copy_row(&mut self, delta: Vector3<i8>, src: &[BlockLight]) {
        self.0.copy_row(delta, src);
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkAreaDataStore<T>([[[T; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl<T> ChunkAreaDataStore<T> {
    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + ChunkArea::PADDING as i8) as usize)
            .into()
    }
}

impl<T: Copy> ChunkAreaDataStore<T> {
    fn copy_row(&mut self, delta: Vector3<i8>, src: &[T]) {
        let [x, y, z] = Self::index_unchecked(delta);
        self.0[x][y][z..][..src.len()].copy_from_slice(src);
    }
}

impl<T> Index<Vector3<i8>> for ChunkAreaDataStore<T> {
    type Output = T;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
pub struct ChunkReach(u32);

impl ChunkReach {
    pub fn insert_block(&mut self, coords: Point3<u8>) {
        self.0 |= Self::block_reach(coords);
    }

    pub fn is_empty(self) -> bool {
        self == Default::default()
    }

    fn block_reach(coords: Point3<u8>) -> u32 {
        const { assert!(ChunkArea::CHUNK_VOLUME <= u32::BITS as usize) };

        Self::axis_reach(coords.x, ChunkArea::CHUNK_DIM.pow(2))
            * Self::axis_reach(coords.y, ChunkArea::CHUNK_DIM)
            * Self::axis_reach(coords.z, 1)
    }

    fn axis_reach(c: u8, stride: usize) -> u32 {
        let c = c as isize;
        let chunk_dim = Chunk::DIM as isize;
        let block_padding = ChunkArea::PADDING as isize;
        let chunk_padding = ChunkArea::CHUNK_PADDING as isize;

        let start = (c - block_padding).div_euclid(chunk_dim);
        let end = (c + block_padding).div_euclid(chunk_dim);
        (start..=end).fold(0, |mask, dc| {
            let axis_index = (dc + chunk_padding) as usize;
            mask | (1 << (axis_index * stride))
        })
    }

    fn index_unchecked(delta: Vector3<i8>) -> usize {
        Self::cuboid().flatten(delta.cast().into()) as usize
    }

    fn cuboid() -> Cuboid {
        Cuboid::unit().pad(ChunkArea::CHUNK_PADDING as i64)
    }
}

impl IntoIterator for ChunkReach {
    type Item = Vector3<i8>;
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        ChunkArea::chunk_deltas().filter(move |&delta| self.0.bit(Self::index_unchecked(delta)))
    }
}

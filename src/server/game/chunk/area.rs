use super::Chunk;
use crate::{
    server::game::block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaView, BlockLightAreaView},
    },
    shared::cuboid::Cuboid,
};
use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};
use std::ops::{Index, Range};

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkArea(ChunkAreaDataStore<Block>);

impl ChunkArea {
    const PADDING: usize = BlockArea::PADDING;
    pub const CHUNK_PADDING: usize = Self::PADDING.div_ceil(Chunk::DIM);
    const DIM: usize = Chunk::DIM + 2 * Self::PADDING;

    pub fn block_area_view(&self, coords: Point3<u8>) -> BlockAreaView<'_> {
        BlockAreaView::new(&self.0, coords)
    }

    pub fn copy_row(&mut self, delta: Vector3<i8>, src: &[Block]) {
        self.0.copy_row(delta, src);
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        Cuboid::unit()
            .pad(Self::CHUNK_PADDING as i64)
            .into_points()
            .map(|coords| coords.coords.cast())
    }

    pub fn axis_range(dc: i32) -> Range<u8> {
        let dim = Chunk::DIM as i32;
        let padding = Self::PADDING as i32;
        let start = (-padding - dc * dim).max(0);
        let end = (dim + padding - dc * dim).min(dim);
        start as u8..end as u8
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

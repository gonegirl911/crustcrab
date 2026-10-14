use super::{Chunk, area::ChunkArea};
use crate::server::game::world::{
    block::{Block, area::BlockArea},
    coords,
};
use nalgebra::{Point3, point};
use rustc_hash::FxHashMap;
use std::{ops::Index, sync::Arc};

#[derive(Default)]
pub struct ChunkStore(pub FxHashMap<Point3<i32>, Arc<Chunk>>);

impl ChunkStore {
    pub fn chunk_area(&self, coords: Point3<i32>) -> ChunkArea {
        let mut value = ChunkArea::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(chunk) = self.0.get(&(coords + delta.cast())) {
                let [dx, dy, dz] = delta.into();
                for x in ChunkArea::axis_range(dx) {
                    for y in ChunkArea::axis_range(dy) {
                        let z = ChunkArea::axis_range(dz);
                        value.copy_row(
                            coords::from_parts(point![dx, dy, dz].cast(), point![x, y, z.start])
                                .coords
                                .cast(),
                            chunk.row(point![x, y, z.start], z.len()),
                        );
                    }
                }
            }
        }
        value
    }

    pub fn block_area(&self, coords: Point3<i64>) -> BlockArea {
        BlockArea::from_fn(|delta| self.block(coords + delta.cast()))
    }

    pub fn block(&self, coords: Point3<i64>) -> Block {
        self.0
            .get(&coords::chunk(coords))
            .map_or_default(|chunk| chunk[coords::block(coords)])
    }
}

impl Index<Point3<i32>> for ChunkStore {
    type Output = Chunk;

    fn index(&self, coords: Point3<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

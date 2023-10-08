use super::{
    action::BlockAction,
    block::{Block, BlockLight},
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        Chunk, ChunkLight,
    },
    ChunkStore,
};
use crate::shared::utils;
use nalgebra::{point, Point3, Vector3};
use rustc_hash::FxHashMap;
use std::{
    collections::hash_map::Entry,
    ops::{Index, IndexMut, Range},
};

#[derive(Default)]
pub struct WorldLight(FxHashMap<Point3<i32>, ChunkLight>);

impl WorldLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let mut value = ChunkAreaLight::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.get(coords + delta) {
                for (coords, delta) in ChunkArea::block_deltas(delta) {
                    value[delta] = light[coords];
                }
            }
        }
        value
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> Vec<Point3<i64>> {
        vec![]
    }

    fn get(&self, coords: Point3<i32>) -> Option<&ChunkLight> {
        self.0.get(&coords)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, ChunkLight> {
        self.0.entry(coords)
    }
}

struct Workstation {
    data: Vec<(Block, BlockLight)>,
    min: Point3<i64>,
    dims: Vector3<i64>,
}

impl Workstation {
    fn populate(&mut self, chunks: &ChunkStore, light: &WorldLight) {
        for chunk_coords in self.chunk_points() {
            match (chunks.get(chunk_coords), light.get(chunk_coords)) {
                (Some(chunk), Some(light)) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))] =
                            (chunk[block_coords], light[block_coords]);
                    }
                }
                (Some(chunk), None) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))].0 = chunk[block_coords];
                    }
                }
                (None, Some(light)) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))].1 = light[block_coords];
                    }
                }
                (None, None) => {}
            }
        }
    }

    fn apply(&self, light: &mut WorldLight) -> Vec<Point3<i64>> {
        let mut changes = vec![];
        for chunk_coords in self.chunk_points() {
            match light.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    for block_coords in self.block_points(chunk_coords) {
                        let coords = utils::coords((chunk_coords, block_coords));
                        if entry.get_mut().set(block_coords, self[coords].1) {
                            changes.push(coords);
                        }
                    }
                    if entry.get().is_empty() {
                        entry.remove();
                    }
                }
                Entry::Vacant(entry) => {
                    let mut light = ChunkLight::default();
                    for block_coords in self.block_points(chunk_coords) {
                        let coords = utils::coords((chunk_coords, block_coords));
                        if light.set(block_coords, self[coords].1) {
                            changes.push(coords)
                        }
                    }
                    if !light.is_empty() {
                        entry.insert(light);
                    }
                }
            }
        }
        changes
    }

    fn chunk_points(&self) -> impl Iterator<Item = Point3<i32>> {
        let min = utils::chunk_coords(self.min);
        let max = utils::chunk_coords(self.max());
        (min.x..=max.x).flat_map(move |x| {
            (min.y..=max.y).flat_map(move |y| (min.z..=max.z).map(move |z| point![x, y, z]))
        })
    }

    fn block_points(&self, coords: Point3<i32>) -> impl Iterator<Item = Point3<u8>> {
        let [x, y, z] = coords.into();
        let min = self.min;
        let max = self.max();
        Self::block_axis_range(x, min.x, max.x).flat_map(move |x| {
            Self::block_axis_range(y, min.y, max.y).flat_map(move |y| {
                Self::block_axis_range(z, min.z, max.z).map(move |z| point![x, y, z])
            })
        })
    }

    fn max(&self) -> Point3<i64> {
        self.min + self.dims - Vector3::repeat(1)
    }

    unsafe fn index_unchecked(&self, coords: Point3<i64>) -> usize {
        let delta = coords - self.min;
        (delta.x * self.dims.y * self.dims.z + delta.y * self.dims.z + delta.z) as usize
    }

    fn block_axis_range(c: i32, min: i64, max: i64) -> Range<u8> {
        if utils::chunk_coords(min) == utils::chunk_coords(max) {
            utils::block_coords(min)..utils::block_coords(max) + 1
        } else if c == utils::chunk_coords(min) {
            utils::block_coords(min)..Chunk::DIM as u8
        } else if c == utils::chunk_coords(max) {
            0..utils::block_coords(max) + 1
        } else {
            0..Chunk::DIM as u8
        }
    }
}

impl Index<Point3<i64>> for Workstation {
    type Output = (Block, BlockLight);

    fn index(&self, coords: Point3<i64>) -> &Self::Output {
        &self.data[unsafe { self.index_unchecked(coords) }]
    }
}

impl IndexMut<Point3<i64>> for Workstation {
    fn index_mut(&mut self, coords: Point3<i64>) -> &mut Self::Output {
        let idx = unsafe { self.index_unchecked(coords) };
        &mut self.data[idx]
    }
}

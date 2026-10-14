pub mod region;

use super::ChunkStore;
use crate::{
    server::game::{
        block::{BlockLight, area::BlockLightArea},
        chunk::{
            ChunkLight,
            area::{ChunkArea, ChunkLightArea, ChunkReach},
        },
        coords,
    },
    shared::pool::{self, JobPool},
};
use nalgebra::{Point3, point};
use region::{RegionInput, RegionOutput, SlotGrid};
use rustc_hash::FxHashMap;
use std::{
    sync::{Arc, LazyLock},
    time::Instant,
};

pub struct WorldLight {
    lights: FxHashMap<Point3<i32>, Arc<ChunkLight>>,
    pool: JobPool<RegionInput, RegionOutput, Point3<i32>>,
    pending: FxHashMap<Point3<i32>, PendingRecord>,
}

impl WorldLight {
    pub fn chunk_light_area(&self, coords: Point3<i32>) -> ChunkLightArea {
        let mut value = ChunkLightArea::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.lights.get(&(coords + delta.cast())) {
                let [dx, dy, dz] = delta.into();
                for x in ChunkArea::axis_range(dx) {
                    for y in ChunkArea::axis_range(dy) {
                        let z = ChunkArea::axis_range(dz);
                        value.copy_row(
                            coords::from_parts(point![dx, dy, dz].cast(), point![x, y, z.start])
                                .coords
                                .cast(),
                            light.row(point![x, y, z.start], z.len()),
                        );
                    }
                }
            }
        }
        value
    }

    pub fn block_light_area(&self, coords: Point3<i64>) -> BlockLightArea {
        BlockLightArea::from_fn(|delta| self.block_light(coords + delta.cast()))
    }

    pub fn extend_placeholders<P>(&mut self, new_surface_points: P)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        for coords in new_surface_points {
            for delta in region::chunk_deltas() {
                if delta.y > 0 {
                    self.lights
                        .entry(coords + delta.cast())
                        .or_insert_with(|| PLACEHOLDER.clone());
                }
            }
        }
    }

    pub fn request_regions<P: IntoIterator<Item = Point3<i32>>>(
        &mut self,
        chunks: &ChunkStore,
        points: P,
        has_priority: bool,
    ) {
        for coords in points {
            self.submit_region(chunks, coords, has_priority);
        }
    }

    pub fn settle(
        &mut self,
        chunks: &ChunkStore,
        deadline: Instant,
    ) -> Vec<(Point3<i32>, ChunkReach)> {
        let mut updates = vec![];
        while !self.pending.is_empty()
            && let Ok(mut output) = self.pool.recv_deadline(deadline)
        {
            let pending = self.pending.remove(&output.coords).unwrap();
            output.has_priority |= pending.has_priority;
            self.apply_output(output, chunks, &mut updates);
        }
        updates
    }

    pub fn is_pending(&self, coords: Point3<i32>) -> bool {
        self.pending.contains_key(&coords)
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.lights
            .get(&coords::chunk(coords))
            .map_or_default(|light| light[coords::block(coords)])
    }

    fn submit_region(&mut self, chunks: &ChunkStore, coords: Point3<i32>, has_priority: bool) {
        let pending = self.pending.entry(coords).or_default();
        pending.has_priority |= has_priority;
        self.pool.submit(
            RegionInput {
                coords,
                chunks: SlotGrid::from_fn(|delta| chunks.0.get(&(coords + delta)).cloned()),
                lights: SlotGrid::from_fn(|delta| self.lights.get(&(coords + delta)).cloned()),
                has_priority,
            },
            pool::SubmitOptions {
                key: Some(coords),
                has_priority: pending.has_priority,
            },
        );
    }

    fn apply_output(
        &mut self,
        RegionOutput {
            coords: output_coords,
            input_chunks,
            input_lights,
            has_priority,
        }: RegionOutput,
        chunks: &ChunkStore,
        updates: &mut Vec<(Point3<i32>, ChunkReach)>,
    ) {
        let is_stale = region::chunk_deltas().any(|delta| {
            let coords = output_coords + delta.cast();
            input_chunks[delta].as_ref().map(Arc::as_ptr) != chunks.0.get(&coords).map(Arc::as_ptr)
                || input_lights[delta].as_ref().map(Arc::as_ptr)
                    != self.lights.get(&coords).map(Arc::as_ptr)
        });
        if is_stale {
            self.submit_region(chunks, output_coords, has_priority);
            return;
        }
    }
}

impl Default for WorldLight {
    fn default() -> Self {
        Self {
            lights: Default::default(),
            pool: JobPool::new(THREAD_HEADROOM, region::compute),
            pending: Default::default(),
        }
    }
}

#[derive(Default)]
struct PendingRecord {
    has_priority: bool,
}

static PLACEHOLDER: LazyLock<Arc<ChunkLight>> = LazyLock::new(|| ChunkLight::placeholder().into());

const THREAD_HEADROOM: usize = 4;

pub mod branch;

use super::{ChunkStore, World, action::BlockAction, height::HeightMap};
use crate::server::game::{
    block::{
        BlockLight,
        area::{BlockArea, BlockLightArea},
        data::{SIDE_DELTAS, Side},
    },
    chunk::{
        Chunk, ChunkLight, ChunkReach,
        area::{ChunkArea, ChunkLightArea},
    },
    coords,
};
use branch::{Branch, LazyBranch, Node};
use nalgebra::{Point3, point};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;
use std::sync::{Arc, LazyLock};

#[derive(Default)]
pub struct WorldLight(FxHashMap<Point3<i32>, Arc<ChunkLight>>);

impl WorldLight {
    pub fn chunk_light_area(&self, coords: Point3<i32>) -> ChunkLightArea {
        let mut value = ChunkLightArea::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.0.get(&(coords + delta)) {
                let [dx, dy, dz] = delta.into();
                for x in ChunkArea::axis_range(dx) {
                    for y in ChunkArea::axis_range(dy) {
                        let z = ChunkArea::axis_range(dz);
                        value.copy_row(
                            coords::from_parts(point![dx, dy, dz], point![x, y, z.start])
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
            for delta in ChunkArea::chunk_deltas() {
                if delta.y > 0 {
                    self.0
                        .entry(coords + delta)
                        .or_insert_with(|| PLACEHOLDER.clone());
                }
            }
        }
    }

    pub fn par_insert_many(
        &mut self,
        chunks: &ChunkStore,
        heights: &HeightMap,
        points: &[Point3<i32>],
    ) -> Vec<(Point3<i32>, ChunkReach)> {
        if points.is_empty() {
            return vec![];
        }

        for coords in points {
            self.0.remove(coords);
        }

        let points_per_branch = points
            .len()
            .div_ceil(rayon::current_num_threads() * BRANCHES_PER_THREAD);

        points
            .par_iter()
            .fold_chunks(
                points_per_branch,
                LazyBranch::default,
                |mut branch, &chunk_coords| {
                    let chunk = &chunks[chunk_coords];
                    let light = self.0.get(&chunk_coords);

                    if chunk.is_glowing() {
                        for (block_coords, block) in Chunk::points().zip(chunk.as_slice()) {
                            let node = Self::node(chunk, light, chunk_coords, block_coords);
                            for (i, c) in BlockLight::TORCHLIGHT_RANGE.zip(block.data().luminance) {
                                branch.insert(i, node.with_value(c));
                            }
                        }
                    }

                    for (side, delta) in *SIDE_DELTAS {
                        let Some(neighbor) = self.0.get(&(chunk_coords + delta.cast())) else {
                            continue;
                        };
                        let component_range =
                            if Self::inherits_skylight(heights, chunk_coords, side) {
                                0..BlockLight::LEN
                            } else {
                                BlockLight::TORCHLIGHT_RANGE
                            };
                        for (block_coords, neighbor_block_coords) in side.block_points() {
                            let node = Self::node(chunk, light, chunk_coords, block_coords);
                            let filter = node.block().data().light_filter;
                            let coords = coords::from_parts(chunk_coords, block_coords);
                            let neighbor_value = neighbor[neighbor_block_coords];
                            component_range
                                .clone()
                                .filter(|i| filter[i % 3])
                                .map(|i| (i, neighbor_value.component(i)))
                                .for_each(|(i, c)| {
                                    let absorption = Self::absorption(coords, i, side.opp(), c);
                                    let value = c.saturating_sub(absorption);
                                    branch.insert(i, node.with_value(value));
                                });
                        }
                    }

                    branch
                },
            )
            .map(|branch| branch.evaluate(chunks, self))
            .reduce(Default::default, Branch::sup)
            .merge(self)
    }

    pub fn apply<A>(&mut self, chunks: &ChunkStore, actions: A) -> Vec<(Point3<i32>, ChunkReach)>
    where
        A: IntoIterator<Item = (Point3<i64>, BlockAction)>,
    {
        let mut branch = Branch::default();
        for (coords, action) in actions {
            match action {
                BlockAction::Place(block) => {
                    branch.place(chunks, self, coords, block.data());
                }
                BlockAction::Destroy => {
                    branch.destroy(chunks, self, coords);
                }
            }
        }
        branch.merge(self)
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&coords::chunk(coords))
            .map_or_default(|light| light[coords::block(coords)])
    }

    fn absorption(coords: Point3<i64>, index: usize, travel: Side, neighbor_value: u8) -> u8 {
        if !BlockLight::SKYLIGHT_RANGE.contains(&index) {
            return 1;
        }

        if travel == Side::Top {
            return neighbor_value;
        }

        if coords.y >= World::Y_RANGE.start as i64 * Chunk::DIM as i64 - BlockArea::PADDING as i64
            && travel == Side::Bottom
            && neighbor_value == BlockLight::COMPONENT_MAX
        {
            0
        } else {
            1
        }
    }

    fn node<'a>(
        chunk: &'a Chunk,
        light: Option<&'a Arc<ChunkLight>>,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
    ) -> Node<'a> {
        Node {
            chunk: Some(chunk),
            light,
            chunk_coords,
            block_coords,
            value: 0,
        }
    }

    fn inherits_skylight(heights: &HeightMap, coords: Point3<i32>, side: Side) -> bool {
        match side {
            Side::Top => coords.y == heights.0[&coords.xz()],
            Side::Bottom => false,
            _ => true,
        }
    }
}

static PLACEHOLDER: LazyLock<Arc<ChunkLight>> = LazyLock::new(|| ChunkLight::placeholder().into());

const BRANCHES_PER_THREAD: usize = 4;

use super::Chunk;
use crate::{
    server::game::{block::Block, coords, world::World},
    shared::{pool::JobPool, utils},
};
use nalgebra::Point3;
use noise::{NoiseFn, Simplex};
use rayon::slice::ParallelSliceMut;
use rustc_hash::FxHashSet;
use std::{collections::VecDeque, time::Instant};

pub struct ChunkGeneratorPool {
    #[expect(clippy::type_complexity)]
    pool: JobPool<Point3<i32>, (Point3<i32>, Option<Box<Chunk>>)>,
    requested: FxHashSet<Point3<i32>>,
    deferred: VecDeque<Point3<i32>>,
    in_flight: usize,
}

impl ChunkGeneratorPool {
    pub fn request<P>(&mut self, points: P, center: Point3<i32>)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        let start = self.deferred.len();
        self.deferred.extend(
            points
                .into_iter()
                .filter(|&coords| self.requested.insert(coords)),
        );
        self.deferred.make_contiguous()[start..]
            .par_sort_unstable_by_key(|&coords| utils::distance_squared(coords, center));
    }

    pub fn anticipate<F>(&mut self, demand: usize, mut is_wanted: F)
    where
        F: FnMut(Point3<i32>) -> bool,
    {
        let in_flight_limit = (IN_FLIGHT_PER_DEMAND * demand as f32) as usize;
        while self.in_flight < in_flight_limit {
            let Some(coords) = self.deferred.pop_front() else {
                return;
            };

            if !is_wanted(coords) {
                self.requested.remove(&coords);
                continue;
            }

            self.pool.submit(coords, false);
            self.in_flight += 1;
        }
    }

    pub fn poll_until(&mut self, deadline: Instant) -> Option<(Point3<i32>, Option<Box<Chunk>>)> {
        if self.in_flight == 0 {
            return None;
        }

        let (coords, chunk) = self.pool.recv_deadline(deadline).ok()?;
        self.in_flight -= 1;
        self.requested.remove(&coords);
        Some((coords, chunk))
    }

    fn generate(generator: &ChunkGenerator, coords: Point3<i32>) -> Option<Box<Chunk>> {
        let mut chunk = Box::new(generator.generate(coords));
        if chunk.is_empty() {
            return None;
        }
        chunk.recompute_visibility_graph();
        Some(chunk)
    }
}

impl Default for ChunkGeneratorPool {
    fn default() -> Self {
        let generator = Default::default();
        Self {
            pool: JobPool::new(move |coords| (coords, Self::generate(&generator, coords))),
            requested: Default::default(),
            deferred: Default::default(),
            in_flight: 0,
        }
    }
}

#[derive(Default)]
struct ChunkGenerator {
    noise: Simplex,
}

impl ChunkGenerator {
    fn generate(&self, coords: Point3<i32>) -> Chunk {
        if !(World::Y_RANGE.start..4).contains(&coords.y) {
            return Default::default();
        }

        Chunk::from_fn(|block_coords| {
            let coords = coords::from_parts(coords, block_coords).cast() / Chunk::DIM as f64;
            if self.noise.get(coords.into()) > 0.0 {
                Block::SAND
            } else {
                Block::AIR
            }
        })
    }
}

const IN_FLIGHT_PER_DEMAND: f32 = 2.0;

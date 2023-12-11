use super::{ChunkStore, World};
use nalgebra::{point, vector, Point2, Point3, Vector2};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, ops::Index};

#[derive(Default)]
pub struct HeightMap(FxHashMap<Point2<i32>, i32>);

impl HeightMap {
    pub fn load_many<'a, I: IntoIterator<Item = &'a Point3<i32>>>(&mut self, loads: I) -> bool {
        loads
            .into_iter()
            .fold(false, |accum, &coords| accum | self.load(coords))
    }

    pub fn unload_many<'a, I>(&mut self, chunks: &ChunkStore, unloads: I) -> bool
    where
        I: IntoIterator<Item = &'a Point3<i32>>,
    {
        unloads
            .into_iter()
            .fold(false, |accum, &coords| accum | self.unload(chunks, coords))
    }

    pub fn placeholders(&self) -> FxHashSet<Point3<i32>> {
        Self::chunk_area_points(self.0.keys().copied())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .flat_map(|coords| {
                let top = self.top(coords);
                let bottom = self.bottom(coords).unwrap_or(top);
                (bottom..=top).map(move |y| point![coords.x, y, coords.y])
            })
            .collect()
    }

    fn load(&mut self, coords: Point3<i32>) -> bool {
        match self.0.entry(coords.xz()) {
            Entry::Occupied(entry) => {
                if *entry.get() < coords.y {
                    *entry.into_mut() = coords.y;
                    true
                } else {
                    false
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(coords.y);
                true
            }
        }
    }

    fn unload(&mut self, chunks: &ChunkStore, coords: Point3<i32>) -> bool {
        if let Entry::Occupied(entry) = self.0.entry(coords.xz()) {
            if *entry.get() == coords.y {
                if let Some(height) = Self::height(chunks, coords) {
                    *entry.into_mut() = height;
                } else {
                    entry.remove();
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn top(&self, coords: Point2<i32>) -> i32 {
        Self::chunk_area_points([coords])
            .filter_map(|coords| self.0.get(&coords))
            .map(|&height| height + 1)
            .max()
            .unwrap_or_else(|| unreachable!())
    }

    fn bottom(&self, coords: Point2<i32>) -> Option<i32> {
        self.0.get(&coords).map(|&height| height + 1)
    }

    fn chunk_area_points<I>(points: I) -> impl Iterator<Item = Point2<i32>>
    where
        I: IntoIterator<Item = Point2<i32>>,
    {
        points
            .into_iter()
            .flat_map(|coords| Self::chunk_deltas().map(move |delta| coords + delta))
    }

    fn height(chunks: &ChunkStore, coords: Point3<i32>) -> Option<i32> {
        (World::Y_RANGE.start..coords.y)
            .rev()
            .find(|&y| chunks.contains(point![coords.x, y, coords.z]))
    }

    fn chunk_deltas() -> impl Iterator<Item = Vector2<i32>> {
        (-1..=1).flat_map(|x| (-1..=1).map(move |y| vector![x, y]))
    }
}

impl Index<Point2<i32>> for HeightMap {
    type Output = i32;

    fn index(&self, coords: Point2<i32>) -> &Self::Output {
        &self.0[&coords]
    }
}

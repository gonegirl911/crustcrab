use nalgebra::{point, vector, Point2, Point3, Vector2};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, ops::Index};

#[derive(Default)]
pub struct HeightMap(FxHashMap<Point2<i32>, i32>);

impl HeightMap {
    pub fn load_placeholders<'a, P>(&mut self, points: P) -> impl Iterator<Item = Point3<i32>> + '_
    where
        P: IntoIterator<Item = &'a Point3<i32>>,
    {
        Self::chunk_area_points(points.into_iter().filter_map(|&coords| self.load(coords)))
            .collect::<FxHashSet<_>>()
            .into_iter()
            .flat_map(|coords| {
                let top = self.top(coords);
                let bottom = self.bottom(coords).unwrap_or(top);
                (bottom..=top).map(move |y| point![coords.x, y, coords.y])
            })
    }

    fn load(&mut self, coords: Point3<i32>) -> Option<Point2<i32>> {
        let xz = coords.xz();
        match self.0.entry(xz) {
            Entry::Occupied(entry) if *entry.get() < coords.y => {
                *entry.into_mut() = coords.y;
                Some(xz)
            }
            Entry::Occupied(_) => None,
            Entry::Vacant(entry) => {
                entry.insert(coords.y);
                Some(xz)
            }
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

    fn chunk_area_points<P>(points: P) -> impl Iterator<Item = Point2<i32>>
    where
        P: IntoIterator<Item = Point2<i32>>,
    {
        points
            .into_iter()
            .flat_map(|coords| Self::chunk_deltas().map(move |delta| coords + delta))
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

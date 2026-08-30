use nalgebra::{Point2, Point3, point};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct HeightMap(pub FxHashMap<Point2<i32>, i32>);

impl HeightMap {
    pub fn load_many<P>(&mut self, points: P) -> Vec<Point3<i32>>
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        points
            .into_iter()
            .filter(|&coords| self.load(coords))
            .map(|coords| coords.xz())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .map(|xz| point![xz.x, self.0[&xz], xz.y])
            .collect()
    }

    fn load(&mut self, coords: Point3<i32>) -> bool {
        let xz = coords.xz();
        match self.0.entry(xz) {
            Entry::Occupied(entry) if *entry.get() < coords.y => {
                *entry.into_mut() = coords.y;
                true
            }
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(coords.y);
                true
            }
        }
    }
}

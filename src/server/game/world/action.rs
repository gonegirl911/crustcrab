use super::block::Block;
use crate::shared::utils;
use nalgebra::Point3;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ActionStore(pub FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    pub fn get(&self, coords: Point3<i64>) -> Option<BlockAction> {
        self.0
            .get(&utils::chunk_coords(coords))?
            .get(&utils::block_coords(coords))
            .copied()
    }

    pub fn chunk_actions(
        &self,
        coords: Point3<i32>,
    ) -> impl Iterator<Item = (Point3<u8>, BlockAction)> {
        self.0
            .get(&coords)
            .into_iter()
            .flatten()
            .map(|(&coords, &action)| (coords, action))
    }

    pub fn insert(&mut self, coords: Point3<i64>, action: BlockAction) {
        self.0
            .entry(utils::chunk_coords(coords))
            .or_default()
            .insert(utils::block_coords(coords), action);
    }
}

impl Extend<(Point3<i64>, BlockAction)> for ActionStore {
    fn extend<I: IntoIterator<Item = (Point3<i64>, BlockAction)>>(&mut self, iter: I) {
        for (coords, action) in iter {
            self.insert(coords, action);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum BlockAction {
    Place(Block),
    Destroy,
}

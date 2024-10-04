use super::block::Block;
use crate::shared::utils;
use nalgebra::Point3;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ActionStore(FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    pub fn actions(&self, coords: Point3<i32>) -> impl Iterator<Item = (Point3<u8>, BlockAction)> {
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

#[derive(Clone, Copy)]
pub enum BlockAction {
    Place(Block),
    Destroy,
}

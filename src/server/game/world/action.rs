use crate::server::game::{block::Block, coords};
use nalgebra::Point3;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ActionStore(pub FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    pub fn get(&self, coords: Point3<i64>) -> Option<BlockAction> {
        self.0
            .get(&coords::chunk(coords))?
            .get(&coords::block(coords))
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
            .entry(coords::chunk(coords))
            .or_default()
            .insert(coords::block(coords), action);
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

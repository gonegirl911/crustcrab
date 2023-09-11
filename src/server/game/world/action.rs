use super::{block::Block, chunk::Chunk};
use crate::shared::{dash::FxDashMap, utils};
use nalgebra::Point3;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ActionStore(FxDashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    pub fn apply_unchecked(&self, coords: Point3<i32>, chunk: &mut Chunk) {
        if let Some(actions) = self.0.get(&coords) {
            for (&coords, action) in &*actions {
                chunk.apply_unchecked(coords, action);
            }
        }
    }

    pub fn insert(&self, coords: Point3<i64>, action: BlockAction) {
        self.0
            .entry(utils::chunk_coords(coords))
            .or_default()
            .insert(utils::block_coords(coords), action);
    }
}

pub enum BlockAction {
    Place(Block),
    Destroy,
}

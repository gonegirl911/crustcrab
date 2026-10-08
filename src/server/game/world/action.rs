use crate::server::game::{block::Block, chunk::Chunk, coords};
use nalgebra::Point3;
use rustc_hash::FxHashMap;

#[derive(Default)]
pub struct ActionStore(pub FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockAction>>);

impl ActionStore {
    pub fn apply(&self, coords: Point3<i32>, chunk: Option<Box<Chunk>>) -> Option<Box<Chunk>> {
        let mut actions = self.chunk_actions(coords);

        let (mut chunk, mut is_modified) = match chunk {
            Some(chunk) => (chunk, false),
            None => {
                if let Some((coords, action)) =
                    actions.find(|&(_, action)| Block::AIR.is_action_valid(action))
                {
                    let mut chunk = Box::<Chunk>::default();
                    chunk.apply_unchecked(coords, action);
                    (chunk, true)
                } else {
                    return None;
                }
            }
        };

        for (coords, action) in actions {
            is_modified |= chunk.apply_unchecked(coords, action);
        }

        if chunk.is_empty() {
            return None;
        }

        if is_modified {
            chunk.recompute_visibility_graph();
        }

        Some(chunk)
    }

    pub fn get(&self, coords: Point3<i64>) -> Option<BlockAction> {
        self.0
            .get(&coords::chunk(coords))?
            .get(&coords::block(coords))
            .copied()
    }

    pub fn insert(&mut self, coords: Point3<i64>, action: BlockAction) {
        self.0
            .entry(coords::chunk(coords))
            .or_default()
            .insert(coords::block(coords), action);
    }

    fn chunk_actions(
        &self,
        coords: Point3<i32>,
    ) -> impl Iterator<Item = (Point3<u8>, BlockAction)> {
        self.0
            .get(&coords)
            .into_iter()
            .flatten()
            .map(|(&coords, &action)| (coords, action))
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

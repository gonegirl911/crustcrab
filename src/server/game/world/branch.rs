use super::{
    ChunkStore, World,
    action::{ActionStore, BlockAction},
};
use crate::server::game::{block::Block, chunk::ChunkReach, coords};
use nalgebra::{Point3, Vector3};
use rustc_hash::FxHashSet;
use std::collections::{VecDeque, hash_map::Entry};

#[derive(Default)]
pub struct Branch {
    actions: ActionStore,
}

impl Branch {
    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if !self.is_action_valid(chunks, coords, normal, action) {
            false
        } else {
            self.execute_actions(chunks, VecDeque::from([(coords, action)]));
            true
        }
    }

    pub fn merge(self, chunks: &mut ChunkStore) -> Changelog {
        let mut hits = vec![];
        let mut inserts = FxHashSet::default();
        let mut removals = FxHashSet::default();
        let mut updates = vec![];

        for (chunk_coords, actions) in self.actions.0 {
            match chunks.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let chunk = entry.get_mut();
                    let mut reach = ChunkReach::default();

                    for (block_coords, action) in actions {
                        if chunk.apply(block_coords, action) {
                            hits.push((coords::from_parts(chunk_coords, block_coords), action));
                            reach.insert_block(block_coords);
                        }
                    }

                    if chunk.is_empty() {
                        entry.remove();
                        removals.insert(chunk_coords);
                    } else {
                        chunk.recompute_visibility_graph();
                    }

                    if !reach.is_empty() {
                        updates.push((chunk_coords, reach));
                    }
                }
                Entry::Vacant(entry) => {
                    let mut actions = actions
                        .into_iter()
                        .filter(|&(_, action)| Block::AIR.is_action_valid(action))
                        .peekable();

                    if actions.peek().is_none() {
                        continue;
                    }

                    let chunk = entry.insert(Default::default());
                    let mut reach = ChunkReach::default();

                    for (block_coords, action) in actions {
                        chunk.apply_unchecked(block_coords, action);
                        hits.push((coords::from_parts(chunk_coords, block_coords), action));
                        reach.insert_block(block_coords);
                    }

                    chunk.recompute_visibility_graph();
                    inserts.insert(chunk_coords);
                    updates.push((chunk_coords, reach));
                }
            }
        }

        Changelog {
            actions: hits,
            inserts,
            removals,
            updates,
        }
    }

    fn is_action_valid(
        &self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        normal: Vector3<i64>,
        action: BlockAction,
    ) -> bool {
        if !World::Y_RANGE.contains(&coords::chunk(coords).y)
            || !self.block(chunks, coords).is_action_valid(action)
        {
            return false;
        }

        if let BlockAction::Place(block) = action
            && let Some(surface) = block.data().valid_surface
            && (normal != Vector3::y() || self.block(chunks, coords - normal) != surface)
        {
            return false;
        }

        true
    }

    fn execute_actions(
        &mut self,
        chunks: &ChunkStore,
        mut actions: VecDeque<(Point3<i64>, BlockAction)>,
    ) {
        while let Some((coords, action)) = actions.pop_front() {
            if action == BlockAction::Destroy {
                let coords = coords + Vector3::y();
                if self.block(chunks, coords).data().valid_surface.is_some() {
                    actions.push_front((coords, BlockAction::Destroy));
                }
            }
            self.actions.insert(coords, action);
        }
    }

    fn block(&self, chunks: &ChunkStore, coords: Point3<i64>) -> Block {
        let mut block = chunks.block(coords);
        if let Some(action) = self.actions.get(coords) {
            block.apply_unchecked(action);
        }
        block
    }
}

pub struct Changelog {
    pub actions: Vec<(Point3<i64>, BlockAction)>,
    pub inserts: FxHashSet<Point3<i32>>,
    pub removals: FxHashSet<Point3<i32>>,
    pub updates: Vec<(Point3<i32>, ChunkReach)>,
}

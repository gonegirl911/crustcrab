use super::{Chunk, ChunkBitSet};
use crate::{
    server::game::world::block::{
        area::BlockArea,
        data::{SIDE_DELTAS, Side},
    },
    shared::enum_map::Enum,
};
use bitfield::{Bit, BitMut};
use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisibilityGraph(u16);

impl VisibilityGraph {
    pub const ALL_CONNECTED: Self = Self(u16::MAX >> 1);

    pub fn compute(opaque_set: ChunkBitSet) -> Self {
        let mut graph = Self(0);
        let mut visited = opaque_set;

        for coords in Chunk::points() {
            if visited.replace(coords, true) {
                continue;
            }

            let mut sides = SideSet::default();
            let mut queue = VecDeque::from([coords]);

            while let Some(coords) = queue.pop_front() {
                for (side, delta) in *SIDE_DELTAS {
                    if Chunk::is_in_bounds(coords.cast() + delta).is_none() {
                        sides.insert(side);
                    }
                }

                for delta in BlockArea::deltas() {
                    if delta != Vector3::zeros()
                        && let Some(neighbor_coords) = Chunk::is_in_bounds(coords.cast() + delta)
                        && !visited.replace(neighbor_coords, true)
                    {
                        queue.push_back(neighbor_coords);
                    }
                }
            }

            for a in Side::variants() {
                for b in Side::variants().skip(a.to_index() + 1) {
                    if sides.contains(a) && sides.contains(b) {
                        graph.set_connected(a, b, true);
                    }
                }
            }
        }

        graph
    }

    pub fn connected(&self, a: Side, b: Side) -> bool {
        self.0.bit(Self::pair_index(a, b))
    }

    fn set_connected(&mut self, a: Side, b: Side, value: bool) {
        self.0.set_bit(Self::pair_index(a, b), value);
    }

    fn pair_index(a: Side, b: Side) -> usize {
        let (a, b) = (a.to_index(), b.to_index());
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        hi * (hi - 1) / 2 + lo
    }
}

#[derive(Clone, Copy, Default)]
pub struct SideSet(u8);

impl SideSet {
    pub fn contains(&self, side: Side) -> bool {
        self.0.bit(side.to_index())
    }

    pub fn insert(&mut self, side: Side) {
        self.0.set_bit(side.to_index(), true);
    }
}

impl IntoIterator for SideSet {
    type Item = Side;
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Enum::variants().filter(move |&side| self.contains(side))
    }
}

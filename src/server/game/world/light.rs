use super::{
    block::{data::BlockData, light::BlockLight, Block, SIDE_DELTAS},
    chunk::{
        light::{ChunkAreaLight, ChunkLight},
        Chunk,
    },
    {BlockAction, ChunkStore, World},
};
use nalgebra::Point3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cmp::Ordering, collections::VecDeque};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let coords = coords.cast() * Chunk::DIM as i64;
        ChunkAreaLight::from_fn(|delta| {
            let coords = coords + delta.cast();
            self.block_light(&LightNode::new(coords))
        })
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i64>> {
        match action {
            BlockAction::Place(block) => self.place(chunks, coords, *block),
            BlockAction::Destroy => self.destroy(chunks, coords),
        }
    }

    fn place(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        self.block_skylight(chunks, coords, block)
            .into_iter()
            .chain(self.place_torchlight(chunks, coords, block))
            .collect()
    }

    fn destroy(&mut self, chunks: &ChunkStore, coords: Point3<i64>) -> FxHashSet<Point3<i64>> {
        self.unblock_skylight(chunks, coords)
            .into_iter()
            .chain(self.destroy_torchlight(chunks, coords))
            .collect()
    }

    fn block_skylight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    fn place_torchlight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        block
            .data()
            .luminance
            .into_iter()
            .zip(BlockLight::TORCHLIGHT_RANGE)
            .flat_map(|(v, i)| self.set_torchlight(chunks, coords, i, v))
            .collect()
    }

    fn unblock_skylight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    fn destroy_torchlight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
    ) -> FxHashSet<Point3<i64>> {
        BlockLight::TORCHLIGHT_RANGE
            .flat_map(|i| self.unset_torchlight(chunks, coords, i))
            .collect()
    }

    fn set_torchlight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let component = self.replace_component(coords, index, value);
        match component.cmp(&value) {
            Ordering::Less => self.spread_component(chunks, coords, index, value),
            Ordering::Equal => Default::default(),
            Ordering::Greater => self.unspread_component(chunks, coords, index, component),
        }
    }

    fn unset_torchlight(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        let component = self.take_component(coords, index);
        if component != 0 {
            self.unspread_component(chunks, coords, index, component)
        } else {
            self.spread_neighbors(chunks, coords, index)
        }
    }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut visits = FxHashSet::from_iter([coords]);

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                if let Some(value) = self.set_component(chunks, coords, index, value - 1) {
                    deq.push_back((coords, value));
                }
            }
        }

        visits
    }

    fn unspread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut visits = FxHashSet::from_iter([coords]);
        let mut sources = vec![];

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                match self.unset_component(chunks, coords, index, value - 1) {
                    Ok(value) => deq.push_back((coords, value)),
                    Err(0) => {}
                    Err(component) => sources.push((coords, component)),
                }
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, component)| self.spread_component(chunks, coords, index, component))
            .chain(visits)
            .collect()
    }

    fn spread_neighbors(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        Self::neighbors(coords)
            .filter_map(|coords| {
                let component = self.component(coords, index);
                (component != 0).then(|| self.spread_component(chunks, coords, index, component))
            })
            .flatten()
            .collect()
    }

    fn replace_component(&mut self, coords: Point3<i64>, index: usize, value: u8) -> u8 {
        self.block_light_mut(&LightNode::new(coords))
            .replace_component(index, value)
    }

    fn take_component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
    }

    fn component(&self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light(&LightNode::new(coords)).component(index)
    }

    fn set_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Option<u8> {
        let node = LightNode::new(coords);
        let block_light = self.block_light_mut(&node);
        let component = block_light.component(index);
        let value = node.apply_filter(chunks, index, value);
        (component < value).then(|| {
            block_light.set_component(index, value);
            value
        })
    }

    fn unset_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Result<u8, u8> {
        let node = LightNode::new(coords);
        let block_light = self.block_light_mut(&node);
        let component = block_light.component(index);
        if component != 0 && component == node.apply_filter(chunks, index, value) {
            block_light.set_component(index, 0);
            Ok(value)
        } else {
            Err(component)
        }
    }

    fn block_light(&self, node: &LightNode) -> BlockLight {
        self.0
            .get(&node.chunk_coords)
            .map_or_else(Default::default, |light| light[node.block_coords])
    }

    fn block_light_mut(&mut self, node: &LightNode) -> &mut BlockLight {
        &mut self.0.entry(node.chunk_coords).or_default()[node.block_coords]
    }

    fn unvisited_neighbors(
        coords: Point3<i64>,
        visits: &mut FxHashSet<Point3<i64>>,
    ) -> impl Iterator<Item = Point3<i64>> + '_ {
        Self::neighbors(coords).filter(|coords| visits.insert(*coords))
    }

    fn neighbors(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
    }
}

struct LightNode {
    chunk_coords: Point3<i32>,
    block_coords: Point3<u8>,
}

impl LightNode {
    fn new(coords: Point3<i64>) -> Self {
        Self {
            chunk_coords: World::chunk_coords(coords),
            block_coords: World::block_coords(coords),
        }
    }

    fn apply_filter(&self, chunks: &ChunkStore, index: usize, value: u8) -> u8 {
        (value as f32 * self.filter(chunks, index)).round() as u8
    }

    fn filter(&self, chunks: &ChunkStore, index: usize) -> f32 {
        self.block_data(chunks).light_filter[index % 3]
    }

    fn block_data(&self, chunks: &ChunkStore) -> &'static BlockData {
        chunks
            .get(self.chunk_coords)
            .map_or(Block::Air, |chunk| chunk[self.block_coords])
            .data()
    }
}

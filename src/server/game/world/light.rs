use super::{
    block::{light::BlockLight, Block, SIDE_DELTAS},
    chunk::light::{ChunkAreaLight, ChunkLight},
    {BlockAction, ChunkStore, World},
};
use nalgebra::Point3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{cmp::Ordering, collections::VecDeque};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let coords = World::coords(coords, Default::default());
        ChunkAreaLight::from_fn(|delta| self.block_light(coords + delta.cast()))
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
        self.block_light_mut(coords).replace_component(index, value)
    }

    fn take_component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
    }

    fn component(&self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light(coords).component(index)
    }

    fn set_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Option<u8> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        let value = Self::apply_filter(chunks, coords, index, value);
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
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        if component != 0 && component == Self::apply_filter(chunks, coords, index, value) {
            block_light.set_component(index, 0);
            Ok(value)
        } else {
            Err(component)
        }
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&World::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[World::block_coords(coords)])
    }

    fn block_light_mut(&mut self, coords: Point3<i64>) -> &mut BlockLight {
        &mut self.0.entry(World::chunk_coords(coords)).or_default()[World::block_coords(coords)]
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

    fn apply_filter(chunks: &ChunkStore, coords: Point3<i64>, index: usize, value: u8) -> u8 {
        (value as f32 * Self::filter(chunks, coords, index)).round() as u8
    }

    fn filter(chunks: &ChunkStore, coords: Point3<i64>, index: usize) -> f32 {
        chunks.block(coords).data().light_filter[index % 3]
    }
}

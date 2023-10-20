use super::{
    action::BlockAction,
    block::{
        area::BlockAreaLight,
        data::{BlockData, SIDE_DELTAS},
        BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        ChunkLight,
    },
    ChunkStore,
};
use crate::shared::utils;
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, VecDeque},
};

#[derive(Default)]
pub struct WorldLight(FxHashMap<Point3<i32>, ChunkLight>);

impl WorldLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let mut value = ChunkAreaLight::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.0.get(&(coords + delta)) {
                for (coords, delta) in ChunkArea::block_deltas(delta) {
                    value[delta] = light[coords];
                }
            }
        }
        value
    }

    pub fn block_area_light(&self, coords: Point3<i64>) -> BlockAreaLight {
        BlockAreaLight::from_fn(|delta| self.block_light(coords + delta.cast()))
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> Vec<Point3<i64>> {
        match action {
            BlockAction::Place(block) => {
                let mut branch = Branch::default();
                branch.place(chunks, self, coords, block.data());
                branch.merge(self)
            }
            BlockAction::Destroy => {
                let mut branch = Branch::default();
                branch.destroy(chunks, self, coords, self.flood(coords));
                branch.merge(self)
            }
        }
    }

    fn flood(&self, coords: Point3<i64>) -> BlockLight {
        Branch::adjacent_points(coords)
            .map(|coords| self.block_light(coords))
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
            .map(|c| c.saturating_sub(1))
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&utils::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[utils::block_coords(coords)])
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, ChunkLight> {
        self.0.entry(coords)
    }
}

#[derive(Default)]
struct Branch(FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockLight>>);

impl Branch {
    fn place(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        data: &BlockData,
    ) {
        for i in BlockLight::TORCHLIGHT_RANGE {
            let value = data.luminance[i % 3];
            self.place_filter(chunks, light, coords, i, value, data.light_filter[i % 3]);
            self.place_component(chunks, light, coords, i, value);
        }
    }

    fn destroy(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        value: BlockLight,
    ) {
        for i in BlockLight::TORCHLIGHT_RANGE {
            self.destroy_component(chunks, light, coords, i, value.component(i));
        }
    }

    fn merge(self, light: &mut WorldLight) -> Vec<Point3<i64>> {
        let mut changes = vec![];
        for (chunk_coords, values) in self.0 {
            match light.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    for (block_coords, value) in values {
                        if entry.get_mut().set(block_coords, value) {
                            changes.push(utils::coords((chunk_coords, block_coords)));
                        }
                    }
                    if entry.get().is_empty() {
                        entry.remove();
                    }
                }
                Entry::Vacant(entry) => {
                    if !values.is_empty() {
                        let light = entry.insert(Default::default());
                        for (block_coords, value) in values {
                            light.set(block_coords, value);
                            changes.push(utils::coords((chunk_coords, block_coords)));
                        }
                    }
                }
            }
        }
        changes
    }

    fn place_filter(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        value: u8,
        filter: u8,
    ) {
        if filter == 0 {
            let component = self.component(light, coords, index);
            if component > value {
                self.set_component(light, coords, index, 0);
                self.unspread_component(chunks, light, coords, index, component);
            }
        }
    }

    fn place_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) {
        if self.component(light, coords, index) < value {
            self.set_component(light, coords, index, value);
            self.spread_component(chunks, light, coords, index, value);
        }
    }

    fn destroy_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) {
        let component = self.component(light, coords, index);
        match component.cmp(&value) {
            Ordering::Less => {
                self.set_component(light, coords, index, value);
                self.spread_component(chunks, light, coords, index, value);
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.set_component(light, coords, index, 0);
                self.unspread_component(chunks, light, coords, index, component);
            }
        }
    }

    fn unspread_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        expected: u8,
    ) {
        let mut deq = VecDeque::from_iter(Self::neighbors(coords, expected));
        let mut sources = vec![];

        while let Some((coords, expected)) = deq.pop_front() {
            let data = chunks.block(coords).data();
            let component = self.component(light, coords, index);
            let expected = expected * data.light_filter[index % 3];
            match component.cmp(&expected) {
                Ordering::Less => {}
                Ordering::Equal => {
                    let luminance = data.luminance[index % 3];
                    if luminance != 0 {
                        self.set_component(light, coords, index, luminance);
                        sources.push((coords, luminance));
                    } else {
                        self.set_component(light, coords, index, 0);
                    }
                    deq.extend(Self::neighbors(coords, expected));
                }
                Ordering::Greater => sources.push((coords, component)),
            }
        }

        for (coords, value) in sources {
            self.spread_component(chunks, light, coords, index, value);
        }
    }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) {
        let mut deq = VecDeque::from_iter(Self::neighbors(coords, value));
        while let Some((coords, value)) = deq.pop_front() {
            let value = value * chunks.block(coords).data().light_filter[index % 3];
            if self.component(light, coords, index) < value {
                self.set_component(light, coords, index, value);
                deq.extend(Self::neighbors(coords, value));
            }
        }
    }

    fn component(&self, light: &WorldLight, coords: Point3<i64>, index: usize) -> u8 {
        self.0
            .get(&utils::chunk_coords(coords))
            .and_then(|light| light.get(&utils::block_coords(coords)))
            .copied()
            .unwrap_or_else(|| light.block_light(coords))
            .component(index)
    }

    fn set_component(&mut self, light: &WorldLight, coords: Point3<i64>, index: usize, value: u8) {
        self.0
            .entry(utils::chunk_coords(coords))
            .or_default()
            .entry(utils::block_coords(coords))
            .or_insert(light.block_light(coords))
            .set_component(index, value);
    }

    fn neighbors(coords: Point3<i64>, value: u8) -> impl Iterator<Item = (Point3<i64>, u8)> {
        (value > 1)
            .then(|| Self::adjacent_points(coords).map(move |coords| (coords, value - 1)))
            .into_iter()
            .flatten()
    }

    fn adjacent_points(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
    }
}

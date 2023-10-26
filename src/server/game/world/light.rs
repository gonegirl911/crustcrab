use super::{
    action::BlockAction,
    block::{
        area::BlockAreaLight,
        data::{BlockData, SIDE_DELTAS},
        Block, BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        Chunk, ChunkLight,
    },
    ChunkStore,
};
use crate::shared::utils;
use nalgebra::Point3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        hash_map::{Entry, VacantEntry},
        VecDeque,
    },
};

#[derive(Default)]
pub struct WorldLight(FxHashMap<Point3<i32>, ChunkLight>);

impl WorldLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let mut value = ChunkAreaLight::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.get(coords + delta) {
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
        action: BlockAction,
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
        Self::adjacent_points(coords)
            .map(|coords| self.block_light(coords))
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
            .map(|c| c.saturating_sub(1))
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.get(utils::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[utils::block_coords(coords)])
    }

    fn get(&self, coords: Point3<i32>) -> Option<&ChunkLight> {
        self.0.get(&coords)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, ChunkLight> {
        self.0.entry(coords)
    }

    fn adjacent_points(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
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
                    let mut non_zero_values = values
                        .into_iter()
                        .filter(|(_, value)| *value != Default::default())
                        .peekable();

                    if non_zero_values.peek().is_some() {
                        let light = entry.insert(Default::default());
                        for (block_coords, value) in non_zero_values {
                            assert!(light.set(block_coords, value));
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
            let node = Node::new(chunks, light, coords);
            let block_light = BlockLightRefMut::new(self, &node);
            let component = block_light.component(index);
            if component > value {
                block_light.set_component(index, 0);
                self.unspread_component(chunks, light, node.with_value(component), index);
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
        if value != 0 {
            let node = Node::new(chunks, light, coords);
            let block_light = BlockLightRefMut::new(self, &node);
            if block_light.component(index) < value {
                block_light.set_component(index, value);
                self.spread_component(chunks, light, node.with_value(value), index);
            }
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
        let node = Node::new(chunks, light, coords);
        let block_light = BlockLightRefMut::new(self, &node);
        let component = block_light.component(index);
        match component.cmp(&value) {
            Ordering::Less => {
                block_light.set_component(index, value);
                self.spread_component(chunks, light, node.with_value(value), index);
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                block_light.set_component(index, 0);
                self.unspread_component(chunks, light, node.with_value(component), index);
            }
        }
    }

    fn unspread_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        node: Node,
        index: usize,
    ) {
        let mut visits = FxHashSet::from_iter([node.coords]);
        let mut deq = VecDeque::from_iter([node]);
        let mut sources = vec![];

        while let Some(node) = deq.pop_front() {
            for node in node.unvisited_neighbors(chunks, light, &mut visits) {
                let block_light = BlockLightRefMut::new(self, &node);
                let component = block_light.component(index);
                let data = node.block().data();
                let expected = node.value * data.light_filter[index % 3];
                match component.cmp(&expected) {
                    Ordering::Less => unreachable!(),
                    Ordering::Equal => {
                        let luminance = data.luminance[index % 3];
                        if luminance != 0 {
                            block_light.set_component(index, luminance);
                            sources.push(node.with_value(luminance));
                        } else {
                            block_light.set_component(index, 0);
                        }
                        deq.push_back(node.with_value(expected));
                    }
                    Ordering::Greater => sources.push(node.with_value(component)),
                }
            }
        }

        for node in sources {
            self.spread_component(chunks, light, node, index);
        }
    }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        node: Node,
        index: usize,
    ) {
        let mut visits = FxHashSet::from_iter([node.coords]);
        let mut deq = VecDeque::from_iter([node]);
        while let Some(node) = deq.pop_front() {
            for node in node.unvisited_neighbors(chunks, light, &mut visits) {
                let block_light = BlockLightRefMut::new(self, &node);
                let value = node.value * node.block().data().light_filter[index % 3];
                if block_light.component(index) < value {
                    block_light.set_component(index, value);
                    deq.push_back(node.with_value(value));
                }
            }
        }
    }

    fn entry(
        &mut self,
        coords: Point3<i32>,
    ) -> Entry<Point3<i32>, FxHashMap<Point3<u8>, BlockLight>> {
        self.0.entry(coords)
    }
}

struct Node<'a> {
    chunk: Option<&'a Chunk>,
    light: Option<&'a ChunkLight>,
    chunk_coords: Point3<i32>,
    block_coords: Point3<u8>,
    coords: Point3<i64>,
    value: u8,
}

impl<'a> Node<'a> {
    fn new(chunks: &'a ChunkStore, light: &'a WorldLight, coords: Point3<i64>) -> Self {
        let chunk_coords = utils::chunk_coords(coords);
        Self {
            chunk: chunks.get(chunk_coords),
            light: light.get(chunk_coords),
            chunk_coords,
            block_coords: utils::block_coords(coords),
            coords,
            value: 0,
        }
    }

    fn with_value(&self, value: u8) -> Self {
        Self { value, ..*self }
    }

    fn block(&self) -> Block {
        self.chunk
            .map_or(Block::Air, |chunk| chunk[self.block_coords])
    }

    fn block_light(&self) -> BlockLight {
        self.light
            .map_or(Default::default(), |light| light[self.block_coords])
    }

    fn unvisited_neighbors<'b>(
        &'b self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        visits: &'b mut FxHashSet<Point3<i64>>,
    ) -> impl Iterator<Item = Node<'a>> + 'b {
        (self.value > 1)
            .then(|| {
                WorldLight::adjacent_points(self.coords)
                    .filter(|&coords| visits.insert(coords))
                    .map(|coords| self.neighbor(chunks, light, coords))
            })
            .into_iter()
            .flatten()
    }

    fn neighbor(&self, chunks: &'a ChunkStore, light: &'a WorldLight, coords: Point3<i64>) -> Self {
        let chunk_coords = utils::chunk_coords(coords);
        if self.chunk_coords == chunk_coords {
            Self {
                block_coords: utils::block_coords(coords),
                coords,
                value: self.value - 1,
                ..*self
            }
        } else {
            Self {
                chunk: chunks.get(chunk_coords),
                light: light.get(chunk_coords),
                chunk_coords,
                block_coords: utils::block_coords(coords),
                coords,
                value: self.value - 1,
            }
        }
    }
}

enum BlockLightRefMut<'a> {
    Init(&'a mut BlockLight),
    UninitChunk {
        entry: VacantEntry<'a, Point3<i32>, FxHashMap<Point3<u8>, BlockLight>>,
        coords: Point3<u8>,
        fallback: BlockLight,
    },
    UninitBlock {
        entry: VacantEntry<'a, Point3<u8>, BlockLight>,
        fallback: BlockLight,
    },
}

impl<'a> BlockLightRefMut<'a> {
    fn new(branch: &'a mut Branch, node: &Node<'a>) -> Self {
        match branch.entry(node.chunk_coords) {
            Entry::Occupied(entry) => match entry.into_mut().entry(node.block_coords) {
                Entry::Occupied(entry) => Self::Init(entry.into_mut()),
                Entry::Vacant(entry) => Self::UninitBlock {
                    entry,
                    fallback: node.block_light(),
                },
            },
            Entry::Vacant(entry) => Self::UninitChunk {
                entry,
                coords: node.block_coords,
                fallback: node.block_light(),
            },
        }
    }

    fn component(&self, index: usize) -> u8 {
        match self {
            Self::Init(light) => light.component(index),
            Self::UninitChunk { fallback, .. } | Self::UninitBlock { fallback, .. } => {
                fallback.component(index)
            }
        }
    }

    fn set_component(self, index: usize, value: u8) {
        match self {
            Self::Init(light) => light.set_component(index, value),
            Self::UninitChunk {
                entry,
                coords,
                mut fallback,
            } => {
                fallback.set_component(index, value);
                entry.insert(FxHashMap::from_iter([(coords, fallback)]));
            }
            Self::UninitBlock { entry, fallback } => {
                entry.insert(fallback).set_component(index, value);
            }
        }
    }
}

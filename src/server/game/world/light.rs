use super::{
    action::BlockAction,
    block::{
        area::BlockAreaLight,
        data::{BlockData, Side, SIDE_DELTAS},
        Block, BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        Chunk, ChunkLight,
    },
    ChunkStore, World,
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

    pub fn load_many<I>(&mut self, chunks: &ChunkStore, points: I) -> Vec<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        vec![]
    }

    pub fn unload_many<I>(&mut self, chunks: &ChunkStore, points: I) -> Vec<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        vec![]
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

    fn get(&self, coords: Point3<i32>) -> Option<&ChunkLight> {
        self.0.get(&coords)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, ChunkLight> {
        self.0.entry(coords)
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.get(utils::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[utils::block_coords(coords)])
    }

    fn flood(&self, coords: Point3<i64>) -> BlockLight {
        Self::adjacent_points(coords)
            .map(|(side, coords)| {
                self.block_light(coords)
                    .imap(|i, c| Self::value(i, coords, side, c))
            })
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
    }

    fn adjacent_points(coords: Point3<i64>) -> impl Iterator<Item = (Side, Point3<i64>)> {
        SIDE_DELTAS
            .into_iter()
            .map(move |(side, delta)| (side, coords + delta.cast()))
    }

    fn absorption(index: usize, coords: Point3<i64>, value: u8, side: Side, target: Side) -> u8 {
        !(Self::is_exposed(index, coords, value) && side == target) as u8
    }

    fn is_exposed(index: usize, coords: Point3<i64>, value: u8) -> bool {
        BlockLight::SKYLIGHT_RANGE.contains(&index)
            && coords.y >= World::Y_RANGE.start as i64 * Chunk::DIM as i64
            && value == BlockLight::COMPONENT_MAX
    }

    fn value(index: usize, coords: Point3<i64>, side: Side, value: u8) -> u8 {
        value.saturating_sub(Self::absorption(index, coords, value, side, Side::Top))
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
        data: BlockData,
    ) {
        for i in BlockLight::SKYLIGHT_RANGE {
            self.place_filter(chunks, light, coords, i, 0, data.light_filter[i % 3]);
        }

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
        for i in BlockLight::SKYLIGHT_RANGE {
            self.place_component(chunks, light, coords, i, value.component(i));
        }

        for i in BlockLight::TORCHLIGHT_RANGE {
            self.destroy_component(chunks, light, coords, i, value.component(i));
        }
    }

    fn merge(self, light: &mut WorldLight) -> Vec<Point3<i64>> {
        let mut hits = vec![];
        for (chunk_coords, values) in self.0 {
            match light.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let light = entry.get_mut();
                    for (block_coords, value) in values {
                        if light.set(block_coords, value) {
                            hits.push(utils::coords((chunk_coords, block_coords)));
                        }
                    }
                    if light.is_empty() {
                        entry.remove();
                    }
                }
                Entry::Vacant(entry) => {
                    let light = entry.insert(Default::default());
                    for (block_coords, value) in values {
                        assert!(light.set(block_coords, value));
                        hits.push(utils::coords((chunk_coords, block_coords)));
                    }
                }
            }
        }
        hits
    }

    fn place_filter(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        index: usize,
        value: u8,
        filter: bool,
    ) {
        if !filter {
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
        let mut deq = VecDeque::from([node]);
        let mut sources = vec![];

        while let Some(node) = deq.pop_front() {
            for node in node.unvisited_neighbors(chunks, light, index, &mut visits) {
                let data = node.block().data();
                let value = Self::value(data, index);
                if data.light_filter[index % 3] {
                    let block_light = BlockLightRefMut::new(self, &node);
                    let component = block_light.component(index);
                    match component.cmp(&node.value) {
                        Ordering::Less => unreachable!(),
                        Ordering::Equal => {
                            block_light.set_component(index, value);
                            if value != 0 {
                                sources.push(node.with_value(value));
                            }
                            deq.push_back(node);
                        }
                        Ordering::Greater => sources.push(node.with_value(component)),
                    }
                } else if value != 0 {
                    sources.push(node.with_value(value));
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
        let mut deq = VecDeque::from([node]);
        while let Some(node) = deq.pop_front() {
            for node in node.unvisited_neighbors(chunks, light, index, &mut visits) {
                if node.block().data().light_filter[index % 3] {
                    let block_light = BlockLightRefMut::new(self, &node);
                    if block_light.component(index) < node.value {
                        block_light.set_component(index, node.value);
                        deq.push_back(node);
                    }
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

    fn value(data: BlockData, index: usize) -> u8 {
        data.luminance[index % 3] * BlockLight::TORCHLIGHT_RANGE.contains(&index) as u8
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

    fn unvisited_neighbors<'b>(
        &'b self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        index: usize,
        visits: &'b mut FxHashSet<Point3<i64>>,
    ) -> impl Iterator<Item = Self> + 'b {
        (self.value > 1)
            .then(|| {
                WorldLight::adjacent_points(self.coords)
                    .filter(|&(_, coords)| visits.insert(coords))
                    .map(move |(side, coords)| self.neighbor(chunks, light, coords, index, side))
            })
            .into_iter()
            .flatten()
    }

    fn block(&self) -> Block {
        self.chunk
            .map_or(Block::Air, |chunk| chunk[self.block_coords])
    }

    fn block_light(&self) -> BlockLight {
        self.light
            .map_or_else(Default::default, |light| light[self.block_coords])
    }

    fn with_value(&self, value: u8) -> Self {
        Self { value, ..*self }
    }

    fn neighbor(
        &self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        coords: Point3<i64>,
        index: usize,
        side: Side,
    ) -> Self {
        let chunk_coords = utils::chunk_coords(coords);
        if self.chunk_coords == chunk_coords {
            Self {
                block_coords: utils::block_coords(coords),
                coords,
                value: self.value(index, side),
                ..*self
            }
        } else {
            Self {
                chunk: chunks.get(chunk_coords),
                light: light.get(chunk_coords),
                chunk_coords,
                block_coords: utils::block_coords(coords),
                coords,
                value: self.value(index, side),
            }
        }
    }

    fn value(&self, index: usize, side: Side) -> u8 {
        self.value - WorldLight::absorption(index, self.coords, self.value, side, Side::Bottom)
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
            Self::Init(light) => light,
            Self::UninitChunk { fallback, .. } | Self::UninitBlock { fallback, .. } => fallback,
        }
        .component(index)
    }

    fn set_component(self, index: usize, value: u8) {
        match self {
            Self::Init(light) => {
                light.set_component(index, value);
            }
            Self::UninitChunk {
                entry,
                coords,
                fallback,
            } => {
                entry.insert(FxHashMap::from_iter([(
                    coords,
                    fallback.with_component(index, value),
                )]));
            }
            Self::UninitBlock { entry, fallback } => {
                entry.insert(fallback).set_component(index, value);
            }
        }
    }
}

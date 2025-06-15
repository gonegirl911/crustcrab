use super::{
    ChunkStore, World,
    action::BlockAction,
    block::{
        Block, BlockLight,
        area::BlockAreaLight,
        data::{BlockData, SIDE_DELTAS, Side},
    },
    chunk::{
        Chunk, ChunkLight,
        area::{ChunkArea, ChunkAreaLight},
    },
    height::HeightMap,
};
use crate::shared::utils;
use nalgebra::Point3;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        VecDeque,
        hash_map::{Entry, VacantEntry},
    },
    ops::Range,
};

#[derive(Default)]
pub struct WorldLight {
    lights: FxHashMap<Point3<i32>, ChunkLight>,
    placeholders: FxHashSet<Point3<i32>>,
}

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

    pub fn extend_placeholders<P: IntoIterator<Item = Point3<i32>>>(&mut self, points: P) {
        for coords in points {
            if self.placeholders.insert(coords) {
                *self.lights.entry(coords).or_default() |= BlockLight::placeholder();
            }
        }
    }

    pub fn par_insert_many(
        &mut self,
        chunks: &ChunkStore,
        heights: &HeightMap,
        points: &[Point3<i32>],
    ) -> Vec<Point3<i64>> {
        if points.is_empty() {
            return vec![];
        }

        for coords in points {
            self.lights.remove(coords);
        }

        let points_per_branch = points.len().div_ceil(rayon::current_num_threads());

        points
            .par_iter()
            .fold_chunks(
                points_per_branch,
                LazyBranch::default,
                |mut branch, &chunk_coords| {
                    let chunk = &chunks[chunk_coords];
                    let light = self.get(chunk_coords);

                    if chunk.is_glowing() {
                        for (block_coords, block) in chunk.blocks() {
                            let node = Self::node(chunk, light, chunk_coords, block_coords);
                            for (i, c) in BlockLight::TORCHLIGHT_RANGE.zip(block.data().luminance) {
                                branch.insert(i, node.with_value(c));
                            }
                        }
                    }

                    for (side, delta) in *SIDE_DELTAS {
                        if let Some(neighbor) = self.get(chunk_coords + delta.cast()) {
                            let skylight_range = Self::skylight_range(heights, chunk_coords, side);
                            for (block_coords, neighbor_block_coords) in side.block_points() {
                                let node = Self::node(chunk, light, chunk_coords, block_coords);
                                let filter = node.block().data().light_filter;
                                let coords = utils::coords((chunk_coords, block_coords));
                                let neighbor_value = neighbor[neighbor_block_coords];
                                skylight_range
                                    .clone()
                                    .chain(BlockLight::TORCHLIGHT_RANGE)
                                    .filter(|i| filter[i % 3])
                                    .map(|i| (i, neighbor_value.component(i)))
                                    .for_each(|(i, c)| {
                                        let value = Self::value(coords, i, side, c);
                                        branch.insert(i, node.with_value(value));
                                    });
                            }
                        }
                    }

                    branch
                },
            )
            .map(|branch| branch.evaluate(chunks, self))
            .reduce(Default::default, Branch::sup)
            .merge(self)
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
        self.lights.get(&coords)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<'_, Point3<i32>, ChunkLight> {
        self.lights.entry(coords)
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.get(utils::chunk_coords(coords))
            .map_or_default(|light| light[utils::block_coords(coords)])
    }

    fn flood(&self, coords: Point3<i64>) -> BlockLight {
        Self::adjacent_points(coords)
            .map(|(side, neighbor_coords)| {
                self.block_light(neighbor_coords)
                    .map(|i, c| Self::value(coords, i, side, c))
            })
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
    }

    fn adjacent_points(coords: Point3<i64>) -> impl Iterator<Item = (Side, Point3<i64>)> {
        SIDE_DELTAS
            .into_iter()
            .map(move |(side, delta)| (side, coords + delta.cast()))
    }

    fn absorption(
        coords: Point3<i64>,
        index: usize,
        is_neighbor_on_top: bool,
        neighbor_value: u8,
    ) -> u8 {
        !Self::is_exposed(coords, index, is_neighbor_on_top, neighbor_value) as u8
    }

    fn node<'a>(
        chunk: &'a Chunk,
        light: Option<&'a ChunkLight>,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
    ) -> Node<'a> {
        Node {
            chunk: Some(chunk),
            light,
            chunk_coords,
            block_coords,
            coords: utils::coords((chunk_coords, block_coords)),
            value: 0,
        }
    }

    fn skylight_range(heights: &HeightMap, coords: Point3<i32>, side: Side) -> Range<usize> {
        if Self::includes_skylight(heights, coords, side) {
            BlockLight::SKYLIGHT_RANGE
        } else {
            0..0
        }
    }

    fn value(coords: Point3<i64>, index: usize, neighbor_side: Side, neighbor_value: u8) -> u8 {
        neighbor_value.saturating_sub(Self::absorption(
            coords,
            index,
            neighbor_side == Side::Top,
            neighbor_value,
        ))
    }

    fn is_exposed(
        coords: Point3<i64>,
        index: usize,
        is_neighbor_on_top: bool,
        neighbor_value: u8,
    ) -> bool {
        BlockLight::SKYLIGHT_RANGE.contains(&index)
            && neighbor_value == BlockLight::COMPONENT_MAX
            && coords.y >= World::Y_RANGE.start as i64 * Chunk::DIM as i64 - 1
            && is_neighbor_on_top
    }

    fn includes_skylight(heights: &HeightMap, coords: Point3<i32>, side: Side) -> bool {
        match side {
            Side::Top => coords.y == heights[coords.xz()],
            Side::Bottom => false,
            _ => true,
        }
    }
}

#[derive(Default)]
struct LazyBranch<'a> {
    branch: Branch,
    nodes: [NodeDeque<'a>; BlockLight::LEN],
}

impl<'a> LazyBranch<'a> {
    fn insert(&mut self, index: usize, node: Node<'a>) {
        if node.set_component(&mut self.branch, index) {
            self.nodes[index].push_back(node);
        }
    }

    fn evaluate(mut self, chunks: &ChunkStore, light: &WorldLight) -> Branch {
        for (i, nodes) in self.nodes.into_iter().enumerate() {
            self.branch.spread_nodes(chunks, light, i, nodes);
        }

        self.branch
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
        for (i, f) in BlockLight::SKYLIGHT_RANGE.zip(data.light_filter) {
            self.place_filter(chunks, light, coords, i, 0, f);
        }

        for ((i, f), c) in BlockLight::TORCHLIGHT_RANGE
            .zip(data.light_filter)
            .zip(data.luminance)
        {
            self.place_filter(chunks, light, coords, i, c, f);
            self.place_component(chunks, light, coords, i, c);
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

    fn sup(mut self, other: Self) -> Self {
        for (chunk_coords, values) in other.0 {
            match self.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    for (block_coords, value) in values {
                        entry
                            .get_mut()
                            .entry(block_coords)
                            .and_modify(|light| *light = light.sup(value))
                            .or_insert(value);
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(values);
                }
            }
        }
        self
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
            let node = Node::new(chunks, light, coords, 0);
            let block_light = BlockLightRefMut::new(self, &node);
            let component = block_light.component(index);
            if component > value {
                block_light.set_component(index, 0);
                self.unspread_node(chunks, light, index, node.with_value(component));
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
        let node = Node::new(chunks, light, coords, value);
        if node.set_component(self, index) {
            self.spread_nodes(chunks, light, index, [node].into());
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
        let node = Node::new(chunks, light, coords, value);
        let block_light = BlockLightRefMut::new(self, &node);
        let component = block_light.component(index);
        match component.cmp(&value) {
            Ordering::Less => {
                block_light.set_component(index, value);
                self.spread_nodes(chunks, light, index, [node].into());
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                block_light.set_component(index, 0);
                self.unspread_node(chunks, light, index, node.with_value(component));
            }
        }
    }

    fn unspread_node(&mut self, chunks: &ChunkStore, light: &WorldLight, index: usize, node: Node) {
        let mut deq = NodeDeque::from([node]);
        let mut sources = NodeSet::default();

        while let Some(node) = deq.pop_front() {
            for node in node.neighbors(chunks, light, index) {
                let data = node.block().data();
                let value = Self::value(data, index);
                if data.light_filter[index % 3] {
                    let block_light = BlockLightRefMut::new(self, &node);
                    let component = block_light.component(index);
                    match component.cmp(&node.value) {
                        Ordering::Less => {}
                        Ordering::Equal => {
                            block_light.set_component(index, value);
                            sources.insert(node.with_value(value));
                            deq.push_back(node);
                        }
                        Ordering::Greater => sources.insert(node.with_value(component)),
                    }
                } else {
                    sources.insert(node.with_value(value));
                }
            }
        }

        self.spread_nodes(chunks, light, index, sources.into());
    }

    fn spread_nodes<'a>(
        &mut self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        index: usize,
        mut deq: NodeDeque<'a>,
    ) {
        while let Some(node) = deq.pop_front() {
            for node in node.neighbors(chunks, light, index) {
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
    ) -> Entry<'_, Point3<i32>, FxHashMap<Point3<u8>, BlockLight>> {
        self.0.entry(coords)
    }

    fn value(data: BlockData, index: usize) -> u8 {
        data.luminance[index % 3] * BlockLight::TORCHLIGHT_RANGE.contains(&index) as u8
    }
}

#[derive(Default)]
struct NodeDeque<'a>(VecDeque<Node<'a>>);

impl<'a> NodeDeque<'a> {
    fn push_back(&mut self, node: Node<'a>) {
        if node.value > 1 {
            self.0.push_back(node);
        }
    }

    fn pop_front(&mut self) -> Option<Node<'a>> {
        self.0.pop_front()
    }
}

impl<'a, const N: usize> From<[Node<'a>; N]> for NodeDeque<'a> {
    fn from(nodes: [Node<'a>; N]) -> Self {
        let mut value = Self::default();
        for node in nodes {
            value.push_back(node);
        }
        value
    }
}

impl<'a> From<NodeSet<'a>> for NodeDeque<'a> {
    fn from(nodes: NodeSet<'a>) -> Self {
        nodes.deq
    }
}

#[derive(Default)]
struct NodeSet<'a> {
    points: FxHashSet<Point3<i64>>,
    deq: NodeDeque<'a>,
}

impl<'a> NodeSet<'a> {
    fn insert(&mut self, node: Node<'a>) {
        if self.points.insert(node.coords) {
            self.deq.push_back(node);
        }
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
    fn new(chunks: &'a ChunkStore, light: &'a WorldLight, coords: Point3<i64>, value: u8) -> Self {
        let chunk_coords = utils::chunk_coords(coords);
        Self {
            chunk: chunks.get(chunk_coords),
            light: light.get(chunk_coords),
            chunk_coords,
            block_coords: utils::block_coords(coords),
            coords,
            value,
        }
    }

    fn set_component(&self, branch: &mut Branch, index: usize) -> bool {
        if self.value != 0 {
            let block_light = BlockLightRefMut::new(branch, self);
            if block_light.component(index) < self.value {
                block_light.set_component(index, self.value);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn neighbors(
        &self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        index: usize,
    ) -> impl Iterator<Item = Self> {
        WorldLight::adjacent_points(self.coords)
            .map(move |(side, coords)| self.neighbor(chunks, light, coords, index, side))
    }

    fn block(&self) -> Block {
        self.chunk.map_or_default(|chunk| chunk[self.block_coords])
    }

    fn block_light(&self) -> BlockLight {
        self.light.map_or_default(|light| light[self.block_coords])
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
        let value = Self::value(coords, index, side, self.value);
        if self.chunk_coords == chunk_coords {
            Self {
                block_coords: utils::block_coords(coords),
                coords,
                value,
                ..*self
            }
        } else {
            Self {
                chunk: chunks.get(chunk_coords),
                light: light.get(chunk_coords),
                chunk_coords,
                block_coords: utils::block_coords(coords),
                coords,
                value,
            }
        }
    }

    fn value(coords: Point3<i64>, index: usize, side: Side, neighbor_value: u8) -> u8 {
        neighbor_value - WorldLight::absorption(coords, index, side == Side::Bottom, neighbor_value)
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
                entry.insert(FromIterator::from_iter([(
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

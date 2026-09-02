use super::{
    ChunkStore, World,
    action::BlockAction,
    block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaLight},
        data::{BlockData, SIDE_DELTAS, Side},
    },
    chunk::{
        Chunk, ChunkLight,
        area::{ChunkArea, ChunkAreaLight},
    },
    height::HeightMap,
};
use crate::shared::{enum_map::Enum, utils};
use nalgebra::Point3;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        VecDeque,
        hash_map::{Entry, VacantEntry},
    },
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

    pub fn extend_placeholders<P>(&mut self, heights: &HeightMap, new_surface_points: P)
    where
        P: IntoIterator<Item = Point3<i32>>,
    {
        for coords in new_surface_points {
            for neighbor_coords in ChunkArea::chunk_points(coords) {
                if let Some(&max_y) = heights.0.get(&neighbor_coords.xz())
                    && neighbor_coords.y > max_y
                {
                    self.0
                        .entry(neighbor_coords)
                        .or_insert_with(ChunkLight::placeholder);
                }
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
            self.0.remove(coords);
        }

        let points_per_branch = points.len().div_ceil(rayon::current_num_threads());

        points
            .par_iter()
            .fold_chunks(
                points_per_branch,
                LazyBranch::default,
                |mut branch, &chunk_coords| {
                    let chunk = &chunks[chunk_coords];
                    let light = self.0.get(&chunk_coords);

                    if chunk.is_glowing() {
                        for (block_coords, &block) in Chunk::points().zip(chunk.as_slice()) {
                            let node = Self::node(chunk, light, chunk_coords, block_coords);
                            for (i, c) in BlockLight::TORCHLIGHT_RANGE.zip(block.data().luminance) {
                                branch.insert(i, node.with_value(c));
                            }
                        }
                    }

                    for (side, delta) in *SIDE_DELTAS {
                        let Some(neighbor) = self.0.get(&(chunk_coords + delta.cast())) else {
                            continue;
                        };
                        let component_range =
                            if Self::inherits_skylight(heights, chunk_coords, side) {
                                0..BlockLight::LEN
                            } else {
                                BlockLight::TORCHLIGHT_RANGE
                            };
                        for (block_coords, neighbor_block_coords) in side.block_points() {
                            let node = Self::node(chunk, light, chunk_coords, block_coords);
                            let filter = node.block().data().light_filter;
                            let coords = utils::coords(chunk_coords, block_coords);
                            let neighbor_value = neighbor[neighbor_block_coords];
                            component_range
                                .clone()
                                .filter(|i| filter[i % 3])
                                .map(|i| (i, neighbor_value.component(i)))
                                .for_each(|(i, c)| {
                                    let absorption = Self::absorption(coords, i, side.opp(), c);
                                    let value = c.saturating_sub(absorption);
                                    branch.insert(i, node.with_value(value));
                                });
                        }
                    }

                    branch
                },
            )
            .map(|branch| branch.evaluate(chunks, self))
            .reduce(Default::default, Branch::sup)
            .merge(self)
    }

    pub fn apply<A>(&mut self, chunks: &ChunkStore, actions: A) -> Vec<Point3<i64>>
    where
        A: IntoIterator<Item = (Point3<i64>, BlockAction)>,
    {
        let mut branch = Branch::default();
        for (coords, action) in actions {
            match action {
                BlockAction::Place(block) => {
                    branch.place(chunks, self, coords, block.data());
                }
                BlockAction::Destroy => {
                    branch.destroy(chunks, self, coords);
                }
            }
        }
        branch.merge(self)
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&utils::chunk_coords(coords))
            .map_or_default(|light| light[utils::block_coords(coords)])
    }

    fn absorption(coords: Point3<i64>, index: usize, travel: Side, neighbor_value: u8) -> u8 {
        if !BlockLight::SKYLIGHT_RANGE.contains(&index) {
            return 1;
        }

        if travel == Side::Top {
            return neighbor_value;
        }

        if coords.y >= World::Y_RANGE.start as i64 * Chunk::DIM as i64 - BlockArea::PADDING as i64
            && travel == Side::Bottom
            && neighbor_value == BlockLight::COMPONENT_MAX
        {
            0
        } else {
            1
        }
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
            value: 0,
        }
    }

    fn inherits_skylight(heights: &HeightMap, coords: Point3<i32>, side: Side) -> bool {
        match side {
            Side::Top => coords.y == heights.0[&coords.xz()],
            Side::Bottom => false,
            _ => true,
        }
    }
}

#[derive(Default)]
struct LazyBranch<'a> {
    branch: Branch,
    nodes: [NodeQueue<'a>; BlockLight::LEN],
}

impl<'a> LazyBranch<'a> {
    fn insert(&mut self, index: usize, node: Node<'a>) {
        if node.set_component(&mut self.branch, index) {
            self.nodes[index].push(node);
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
struct Branch {
    values: FxHashMap<Point3<i32>, FxHashMap<Point3<u8>, BlockLight>>,
}

impl Branch {
    fn place(
        &mut self,
        chunks: &ChunkStore,
        light: &WorldLight,
        coords: Point3<i64>,
        data: &BlockData,
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

    fn destroy(&mut self, chunks: &ChunkStore, light: &WorldLight, coords: Point3<i64>) {
        let value = self.flood(light, coords);

        for i in BlockLight::SKYLIGHT_RANGE {
            self.place_component(chunks, light, coords, i, value.component(i));
        }

        for i in BlockLight::TORCHLIGHT_RANGE {
            self.destroy_component(chunks, light, coords, i, value.component(i));
        }
    }

    fn sup(mut self, other: Self) -> Self {
        for (chunk_coords, values) in other.values {
            match self.values.entry(chunk_coords) {
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
        for (chunk_coords, values) in self.values {
            match light.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let light = entry.get_mut();
                    for (block_coords, value) in values {
                        if light.set(block_coords, value) {
                            hits.push(utils::coords(chunk_coords, block_coords));
                        }
                    }
                    if light.is_empty() {
                        entry.remove();
                    }
                }
                Entry::Vacant(entry) => {
                    let mut values = values
                        .into_iter()
                        .filter(|(_, value)| *value != Default::default())
                        .peekable();

                    if values.peek().is_some() {
                        let light = entry.insert(Default::default());
                        for (block_coords, value) in values {
                            light.set_unchecked(block_coords, value);
                            hits.push(utils::coords(chunk_coords, block_coords));
                        }
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
            let node = Self::node(chunks, light, coords, 0);
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
        let node = Self::node(chunks, light, coords, value);
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
        let node = Self::node(chunks, light, coords, value);
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

    fn flood(&self, light: &WorldLight, coords: Point3<i64>) -> BlockLight {
        SIDE_DELTAS
            .into_iter()
            .map(|(side, delta)| {
                let neighbor_coords = coords + delta.cast();
                self.block_light(light, neighbor_coords).map(|i, c| {
                    let absorption = WorldLight::absorption(coords, i, side.opp(), c);
                    c.saturating_sub(absorption)
                })
            })
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
    }

    fn unspread_node(&mut self, chunks: &ChunkStore, light: &WorldLight, index: usize, node: Node) {
        let mut queue = NodeQueue::from([node]);
        let mut sources = NodeSet::default();

        while let Some(node) = queue.pop() {
            for node in node.neighbors(chunks, light, index) {
                let data = node.block().data();
                let luminance = Self::luminance(data, index);
                if data.light_filter[index % 3] {
                    let block_light = BlockLightRefMut::new(self, &node);
                    let component = block_light.component(index);
                    match component.cmp(&node.value) {
                        Ordering::Less => {}
                        Ordering::Equal => {
                            block_light.set_component(index, luminance);
                            sources.insert(node.with_value(luminance));
                            queue.push(node);
                        }
                        Ordering::Greater => {
                            sources.insert(node.with_value(component));
                        }
                    }
                } else {
                    sources.insert(node.with_value(luminance));
                }
            }
        }

        sources.retain(|node| {
            self.values
                .get(&node.chunk_coords)
                .and_then(|values| values.get(&node.block_coords))
                .is_none_or(|value| value.component(index) == node.value)
        });

        self.spread_nodes(chunks, light, index, sources.into());
    }

    fn spread_nodes<'a>(
        &mut self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        index: usize,
        mut deq: NodeQueue<'a>,
    ) {
        while let Some(node) = deq.pop() {
            for node in node.neighbors(chunks, light, index) {
                if node.block().data().light_filter[index % 3] && node.set_component(self, index) {
                    deq.push(node);
                }
            }
        }
    }

    fn block_light(&self, light: &WorldLight, coords: Point3<i64>) -> BlockLight {
        self.values
            .get(&utils::chunk_coords(coords))
            .and_then(|values| values.get(&utils::block_coords(coords)))
            .copied()
            .unwrap_or_else(|| light.block_light(coords))
    }

    fn node<'a>(
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        coords: Point3<i64>,
        value: u8,
    ) -> Node<'a> {
        let chunk_coords = utils::chunk_coords(coords);
        Node {
            chunk: chunks.get(chunk_coords),
            light: light.0.get(&chunk_coords),
            chunk_coords,
            block_coords: utils::block_coords(coords),
            value,
        }
    }

    fn luminance(data: &BlockData, index: usize) -> u8 {
        if BlockLight::TORCHLIGHT_RANGE.contains(&index) {
            data.luminance[index % 3]
        } else {
            0
        }
    }
}

#[derive(Default)]
struct NodeQueue<'a>(VecDeque<Node<'a>>);

impl<'a> NodeQueue<'a> {
    fn push(&mut self, node: Node<'a>) -> bool {
        if node.value > 1 {
            self.0.push_back(node);
            true
        } else {
            false
        }
    }

    fn pop(&mut self) -> Option<Node<'a>> {
        self.0.pop_front()
    }
}

impl<'a, const N: usize> From<[Node<'a>; N]> for NodeQueue<'a> {
    fn from(nodes: [Node<'a>; N]) -> Self {
        Self(nodes.into())
    }
}

impl<'a> From<NodeSet<'a>> for NodeQueue<'a> {
    fn from(set: NodeSet<'a>) -> Self {
        set.queue
    }
}

#[derive(Default)]
struct NodeSet<'a> {
    points: FxHashSet<Point3<i64>>,
    queue: NodeQueue<'a>,
}

impl<'a> NodeSet<'a> {
    fn insert(&mut self, node: Node<'a>) -> bool {
        self.points.insert(node.coords()) && self.queue.push(node)
    }

    fn retain<F: FnMut(&Node) -> bool>(&mut self, f: F) {
        self.queue.0.retain(f);
    }
}

struct Node<'a> {
    chunk: Option<&'a Chunk>,
    light: Option<&'a ChunkLight>,
    chunk_coords: Point3<i32>,
    block_coords: Point3<u8>,
    value: u8,
}

impl<'a> Node<'a> {
    fn with_value(&self, value: u8) -> Self {
        Self { value, ..*self }
    }

    fn set_component(&self, branch: &mut Branch, index: usize) -> bool {
        if self.value == 0 {
            return false;
        }

        let block_light = BlockLightRefMut::new(branch, self);
        if block_light.component(index) < self.value {
            block_light.set_component(index, self.value);
            true
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
        Enum::variants().map(move |side| self.neighbor(chunks, light, side, index))
    }

    fn block(&self) -> Block {
        self.chunk.map_or_default(|chunk| chunk[self.block_coords])
    }

    fn block_light(&self) -> BlockLight {
        self.light.map_or_default(|light| light[self.block_coords])
    }

    fn coords(&self) -> Point3<i64> {
        utils::coords(self.chunk_coords, self.block_coords)
    }

    fn neighbor(
        &self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        side: Side,
        index: usize,
    ) -> Self {
        let coords = self.coords() + SIDE_DELTAS[side].cast();
        let chunk_coords = utils::chunk_coords(coords);
        let block_coords = utils::block_coords(coords);
        let absorption = WorldLight::absorption(coords, index, side, self.value);
        let value = self.value.saturating_sub(absorption);
        if self.chunk_coords == chunk_coords {
            Self {
                block_coords,
                value,
                ..*self
            }
        } else {
            Self {
                chunk: chunks.get(chunk_coords),
                light: light.0.get(&chunk_coords),
                chunk_coords,
                block_coords,
                value,
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
        match branch.values.entry(node.chunk_coords) {
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
                mut fallback,
            } => {
                fallback.set_component(index, value);
                entry.insert([(coords, fallback)].into_iter().collect());
            }
            Self::UninitBlock { entry, fallback } => {
                entry.insert(fallback).set_component(index, value);
            }
        }
    }
}

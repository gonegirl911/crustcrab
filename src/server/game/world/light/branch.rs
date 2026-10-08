use super::WorldLight;
use crate::{
    server::game::{
        block::{
            Block, BlockLight,
            data::{BlockData, SIDE_DELTAS, Side},
        },
        chunk::{Chunk, ChunkLight, ChunkReach},
        coords,
        world::ChunkStore,
    },
    shared::enum_map::Enum,
};
use nalgebra::Point3;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp,
    collections::{VecDeque, hash_map::Entry},
    sync::{Arc, LazyLock},
};

#[derive(Default)]
pub struct LazyBranch<'a> {
    branch: Branch,
    nodes: [NodeQueue<'a>; BlockLight::LEN],
}

impl<'a> LazyBranch<'a> {
    pub fn insert(&mut self, index: usize, node: Node<'a>) {
        if node.set_component(&mut self.branch, index) {
            self.nodes[index].push(node);
        }
    }

    pub fn evaluate(mut self, chunks: &ChunkStore, light: &WorldLight) -> Branch {
        for (i, nodes) in self.nodes.into_iter().enumerate() {
            self.branch.spread_nodes(chunks, light, i, nodes);
        }

        self.branch
    }
}

#[derive(Default)]
pub struct Branch {
    values: FxHashMap<Point3<i32>, Arc<ChunkLight>>,
}

impl Branch {
    pub fn place(
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

    pub fn destroy(&mut self, chunks: &ChunkStore, light: &WorldLight, coords: Point3<i64>) {
        let value = self.flood(light, coords);

        for i in BlockLight::SKYLIGHT_RANGE {
            self.place_component(chunks, light, coords, i, value.component(i));
        }

        for i in BlockLight::TORCHLIGHT_RANGE {
            self.destroy_component(chunks, light, coords, i, value.component(i));
        }
    }

    pub fn sup(mut self, other: Self) -> Self {
        for (chunk_coords, other) in other.values {
            match self.values.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let values = entry.get_mut();

                    if Arc::ptr_eq(values, &other) || other.is_empty() {
                        continue;
                    }

                    if values.is_empty() {
                        *values = other;
                        continue;
                    }

                    let values = Arc::make_mut(entry.get_mut());
                    *values = ChunkLight::from_fn(|block_coords| {
                        values[block_coords].sup(other[block_coords])
                    });
                }
                Entry::Vacant(entry) => {
                    entry.insert(other);
                }
            }
        }
        self
    }

    pub fn merge(self, light: &mut WorldLight) -> Vec<(Point3<i32>, ChunkReach)> {
        let mut updates = vec![];
        for (chunk_coords, values) in self.values {
            match light.0.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    let light = entry.get_mut();

                    if Arc::ptr_eq(light, &values) {
                        continue;
                    }

                    if let Some(reach) = light.diff_reach(&values) {
                        updates.push((chunk_coords, reach));
                    }

                    if values.is_empty() {
                        entry.remove();
                    } else {
                        *light = values;
                    }
                }
                Entry::Vacant(entry) => {
                    if values.is_empty() {
                        continue;
                    }

                    if let Some(reach) = DEFAULT_CHUNK_LIGHT.diff_reach(&values) {
                        updates.push((chunk_coords, reach));
                    }

                    entry.insert(values);
                }
            }
        }
        updates
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
        if filter {
            return;
        }

        let node = Self::node(chunks, light, coords, 0);
        let block_light = BlockLightRefMut::new(self, &node);
        let component = block_light.component(index);
        if component > value {
            block_light.set_component(index, 0);
            self.unspread_node(chunks, light, index, node.with_value(component));
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
            cmp::Ordering::Less => {
                block_light.set_component(index, value);
                self.spread_nodes(chunks, light, index, [node].into());
            }
            cmp::Ordering::Equal => {}
            cmp::Ordering::Greater => {
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
            .unwrap()
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
                        cmp::Ordering::Less => {}
                        cmp::Ordering::Equal => {
                            block_light.set_component(index, luminance);
                            sources.insert(node.with_value(luminance));
                            queue.push(node);
                        }
                        cmp::Ordering::Greater => {
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
                .map(|value| value[node.block_coords])
                .is_none_or(|value| value.component(index) == node.value)
        });

        self.spread_nodes(chunks, light, index, sources.into())
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

    fn chunk_light_mut(&mut self, node: &Node) -> &mut Arc<ChunkLight> {
        self.values.entry(node.chunk_coords).or_insert_with(|| {
            node.light
                .cloned()
                .unwrap_or_else(|| DEFAULT_CHUNK_LIGHT.clone())
        })
    }

    fn block_light(&self, light: &WorldLight, coords: Point3<i64>) -> BlockLight {
        if let Some(values) = self.values.get(&coords::chunk(coords)) {
            values[coords::block(coords)]
        } else {
            light.block_light(coords)
        }
    }

    fn node<'a>(
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        coords: Point3<i64>,
        value: u8,
    ) -> Node<'a> {
        let chunk_coords = coords::chunk(coords);
        Node {
            chunk: chunks.get(chunk_coords),
            light: light.0.get(&chunk_coords),
            chunk_coords,
            block_coords: coords::block(coords),
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
        self.queue.0.retain(f)
    }
}

#[derive(Clone, Copy)]
pub struct Node<'a> {
    pub chunk: Option<&'a Chunk>,
    pub light: Option<&'a Arc<ChunkLight>>,
    pub chunk_coords: Point3<i32>,
    pub block_coords: Point3<u8>,
    pub value: u8,
}

impl<'a> Node<'a> {
    pub fn with_value(&self, value: u8) -> Self {
        Self { value, ..*self }
    }

    pub fn block(&self) -> Block {
        self.chunk.map_or_default(|chunk| chunk[self.block_coords])
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

    fn coords(&self) -> Point3<i64> {
        coords::from_parts(self.chunk_coords, self.block_coords)
    }

    fn neighbor(
        &self,
        chunks: &'a ChunkStore,
        light: &'a WorldLight,
        side: Side,
        index: usize,
    ) -> Self {
        let coords = self.coords() + SIDE_DELTAS[side].cast();
        let chunk_coords = coords::chunk(coords);
        let block_coords = coords::block(coords);
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

struct BlockLightRefMut<'a> {
    chunk_light: &'a mut Arc<ChunkLight>,
    coords: Point3<u8>,
}

impl<'a> BlockLightRefMut<'a> {
    fn new(branch: &'a mut Branch, node: &Node<'a>) -> Self {
        Self {
            chunk_light: branch.chunk_light_mut(node),
            coords: node.block_coords,
        }
    }

    fn component(&self, index: usize) -> u8 {
        self.chunk_light[self.coords].component(index)
    }

    fn set_component(self, index: usize, value: u8) {
        let mut block_light = self.chunk_light[self.coords];
        if block_light.component(index) != value {
            block_light.set_component(index, value);
            Arc::make_mut(self.chunk_light).set(self.coords, block_light);
        }
    }
}

static DEFAULT_CHUNK_LIGHT: LazyLock<Arc<ChunkLight>> = LazyLock::new(Arc::default);

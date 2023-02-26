use super::{
    block::{Block, BlockArea, BlockData, Corner, Side, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS},
    chunk::{BlockAction, Chunk, ChunkArea, ChunkCell, Permutation},
};
use crate::server::game::player::Player;
use bitfield::bitfield;
use enum_map::EnumMap;
use nalgebra::{point, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    cmp::Ordering,
    collections::VecDeque,
    iter::Sum,
    ops::{Index, IndexMut},
};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        ChunkAreaLight::new(&self.0, coords)
    }

    pub fn apply(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i32>> {
        match action {
            BlockAction::Destroy => self.destroy(cells, coords),
            BlockAction::Place(block) => self.place(cells, coords, *block),
        }
    }

    fn destroy(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> FxHashSet<Point3<i32>> {
        (0..3)
            .flat_map(|i| self.destroy_channel(cells, coords, i))
            .collect()
    }

    fn place(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        block
            .data()
            .luminance()
            .into_iter()
            .enumerate()
            .flat_map(|(i, v)| self.place_channel(cells, coords, i, v))
            .collect()
    }

    fn place_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let channel = self.replace_channel(coords, index, value);
        match channel.cmp(&value) {
            Ordering::Less => self.spread_channel(cells, coords, index, value),
            Ordering::Equal => Default::default(),
            Ordering::Greater => self.unspread_channel(cells, coords, index, channel),
        }
    }

    fn destroy_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        let channel = self.replace_channel(coords, index, 0);
        if channel != 0 {
            self.unspread_channel(cells, coords, index, channel)
        } else {
            self.spread_neighbors(cells, coords, index)
        }
    }

    fn spread_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = Self::neighbors(coords, value - 1).collect::<VecDeque<_>>();
        let mut updates = FxHashSet::default();

        while let Some(node) = deq.pop_front() {
            updates.insert(node.chunk_coords);
            if let Some(value) = self.set_channel(cells, &node, index) {
                deq.extend(Self::neighbors(node.coords, value - 1));
            }
        }

        updates
    }

    fn unspread_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = Self::neighbors(coords, value - 1).collect::<VecDeque<_>>();
        let mut sources = vec![];
        let mut updates = FxHashSet::default();

        while let Some(node) = deq.pop_front() {
            updates.insert(node.chunk_coords);
            match self.unset_channel(cells, &node, index) {
                Ok(channel) => deq.extend(Self::neighbors(node.coords, channel - 1)),
                Err(0) => {}
                Err(channel) => sources.push((node.coords, channel)),
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, channel)| self.spread_channel(cells, coords, index, channel))
            .chain(updates)
            .collect()
    }

    fn spread_neighbors(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        SIDE_DELTAS
            .values()
            .filter_map(|delta| {
                let coords = coords + delta.coords.cast();
                let channel = self.channel(coords, index);
                (channel != 0).then(|| self.spread_channel(cells, coords, index, channel))
            })
            .flatten()
            .collect()
    }

    fn replace_channel(&mut self, coords: Point3<f32>, index: usize, value: u8) -> u8 {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        self.block_light_mut(chunk_coords, block_coords)
            .replace_channel(index, value)
    }

    fn channel(&mut self, coords: Point3<f32>, index: usize) -> u8 {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        self.block_light_mut(chunk_coords, block_coords)
            .channel(index)
    }

    fn set_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        node: &LightNode,
        index: usize,
    ) -> Option<u8> {
        let block_light = self.block_light_mut(node.chunk_coords, node.block_coords);
        let channel = block_light.channel(index);
        let value = node.filtered_value(cells, index);
        (channel < value).then(|| {
            block_light.set_channel(index, value);
            value
        })
    }

    fn unset_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        node: &LightNode,
        index: usize,
    ) -> Result<u8, u8> {
        let block_light = self.block_light_mut(node.chunk_coords, node.block_coords);
        let channel = block_light.channel(index);
        if channel != 0 && channel == node.filtered_value(cells, index) {
            block_light.set_channel(index, 0);
            Ok(channel)
        } else {
            Err(channel)
        }
    }

    fn block_light_mut(
        &mut self,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
    ) -> &mut BlockLight {
        &mut self.0.entry(chunk_coords).or_default()[block_coords]
    }

    fn neighbors(coords: Point3<f32>, value: u8) -> impl Iterator<Item = LightNode> {
        SIDE_DELTAS
            .values()
            .map(move |delta| LightNode::new(coords + delta.coords.cast(), value))
    }
}

#[derive(Default)]
struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl ChunkLight {
    fn block_lights(&self) -> impl Iterator<Item = (Point3<u8>, &BlockLight)> + '_ {
        self.0.iter().zip(0..).flat_map(move |(block_lights, x)| {
            block_lights
                .iter()
                .zip(0..)
                .flat_map(move |(block_lights, y)| {
                    block_lights
                        .iter()
                        .zip(0..)
                        .map(move |(block_light, z)| (point![x, y, z], block_light))
                })
        })
    }
}

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for ChunkLight {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

pub struct ChunkAreaLight([[[BlockLight; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl ChunkAreaLight {
    fn new(chunk_lights: &FxHashMap<Point3<i32>, ChunkLight>, coords: Point3<i32>) -> Self {
        let mut value = Self(Default::default());

        if let Some(chunk_light) = chunk_lights.get(&coords) {
            for (delta, block_light) in chunk_light.block_lights() {
                value[delta.cast()] = *block_light;
            }
        }

        for perm in [
            Permutation([0, 1, 2]),
            Permutation([1, 0, 2]),
            Permutation([1, 2, 0]),
        ] {
            for x in [-1, Chunk::DIM as i8] {
                let delta = perm * point![x, 0, 0];
                let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                let block_coords = Player::block_coords(delta.cast());
                let Some(chunk_light) = chunk_lights.get(&chunk_coords) else { continue };
                for y in 0..Chunk::DIM as u8 {
                    for z in 0..Chunk::DIM as u8 {
                        let rest = perm * point![0, y, z];
                        let delta = delta + rest.coords.cast();
                        let block_coords = block_coords + rest.coords;
                        value[delta] = chunk_light[block_coords];
                    }
                }
            }
        }

        for perm in [
            Permutation([0, 1, 2]),
            Permutation([0, 2, 1]),
            Permutation([2, 0, 1]),
        ] {
            for x in [-1, Chunk::DIM as i8] {
                for y in [-1, Chunk::DIM as i8] {
                    let delta = perm * point![x, y, 0];
                    let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                    let block_coords = Player::block_coords(delta.cast());
                    let Some(chunk_light) = chunk_lights.get(&chunk_coords) else { continue };
                    for z in 0..Chunk::DIM as u8 {
                        let rest = perm * point![0, 0, z];
                        let delta = delta + rest.coords.cast();
                        let block_coords = block_coords + rest.coords;
                        value[delta] = chunk_light[block_coords];
                    }
                }
            }
        }

        for x in [-1, Chunk::DIM as i8] {
            for y in [-1, Chunk::DIM as i8] {
                for z in [-1, Chunk::DIM as i8] {
                    let delta = point![x, y, z];
                    let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
                    let block_coords = Player::block_coords(delta.cast());
                    let Some(chunk_light) = chunk_lights.get(&chunk_coords) else { continue };
                    value[delta] = chunk_light[block_coords];
                }
            }
        }

        value
    }

    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        let coords = coords.cast();
        BlockAreaLight::from_fn(|delta| self[coords + delta.coords])
    }
}

impl Index<Point3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c + 1);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<i8>> for ChunkAreaLight {
    fn index_mut(&mut self, coords: Point3<i8>) -> &mut Self::Output {
        let coords = coords.map(|c| c + 1);
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct BlockLight(u16);
    u8;
    component, set_component: 3, 0, 4;
    skylight, set_skylight: 3, 0;
    channel, set_channel: 7, 4, 3;
}

impl BlockLight {
    fn replace_channel(&mut self, index: usize, value: u8) -> u8 {
        let channel = self.channel(index);
        self.set_channel(index, value);
        channel
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 - 1))))
        }))
    }

    pub fn corner_lights(&self, side: Side) -> EnumMap<Corner, BlockLight> {
        let side_light = self[SIDE_DELTAS[side]];
        SIDE_CORNER_COMPONENT_DELTAS[side].map(|_, component_deltas| {
            component_deltas
                .into_values()
                .map(|delta| self[delta])
                .chain([side_light])
                .sum::<BlockLightSum>()
                .avg()
        })
    }
}

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c + 1);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

struct BlockLightSum([(u8, u8); 4]);

impl BlockLightSum {
    fn avg(self) -> BlockLight {
        let mut value = BlockLight::default();
        for (i, (sum, count)) in self.0.into_iter().enumerate() {
            value.set_component(i, sum / count.max(1))
        }
        value
    }
}

impl Sum<BlockLight> for BlockLightSum {
    fn sum<I: Iterator<Item = BlockLight>>(iter: I) -> Self {
        Self(iter.fold(Default::default(), |accum, light| {
            array::from_fn(|i| {
                let (sum, count) = accum[i];
                let component = light.component(i);
                (sum + component, count + (component != 0) as u8)
            })
        }))
    }
}

struct LightNode {
    coords: Point3<f32>,
    chunk_coords: Point3<i32>,
    block_coords: Point3<u8>,
    value: u8,
}

impl LightNode {
    fn new(coords: Point3<f32>, value: u8) -> Self {
        Self {
            coords,
            chunk_coords: Player::chunk_coords(coords),
            block_coords: Player::block_coords(coords),
            value,
        }
    }

    fn filtered_value(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>, index: usize) -> u8 {
        (self.value as f32 * self.filter(cells, index)).round() as u8
    }

    fn filter(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>, index: usize) -> f32 {
        self.block_data(cells).light_filter()[index]
    }

    fn block_data(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>) -> &'static BlockData {
        cells
            .get(&self.chunk_coords)
            .map_or(Block::Air, |cell| cell[self.block_coords])
            .data()
    }
}

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
    ops::{Index, IndexMut, Range},
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
        self.place_skylight(cells, coords)
            .into_iter()
            .chain(self.destroy_torchlight(cells, coords))
            .collect()
    }

    fn place(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        self.destroy_skylight(cells, coords, block)
            .into_iter()
            .chain(self.place_torchlight(cells, coords, block))
            .collect()
    }

    fn place_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> FxHashSet<Point3<i32>> {
        todo!()
    }

    fn place_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        block
            .data()
            .luminance()
            .into_iter()
            .zip(BlockLight::TORCHLIGHT_RANGE)
            .flat_map(|(v, i)| self.set_torchlight(cells, coords, i, v))
            .collect()
    }

    fn destroy_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        todo!()
    }

    fn destroy_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> FxHashSet<Point3<i32>> {
        BlockLight::TORCHLIGHT_RANGE
            .flat_map(|i| self.unset_torchlight(cells, coords, i))
            .collect()
    }

    fn set_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        todo!()
    }

    fn set_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let component = self.replace_component(coords, index, value);
        match component.cmp(&value) {
            Ordering::Less => self.spread_component(cells, coords, index, value),
            Ordering::Equal => Default::default(),
            Ordering::Greater => self.unspread_component(cells, coords, index, component),
        }
    }

    fn unset_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        todo!()
    }

    fn unset_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        let component = self.take_component(coords, index);
        if component != 0 {
            self.unspread_component(cells, coords, index, component)
        } else {
            self.spread_neighbors(cells, coords, index)
        }
    }

    fn spread_component(
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
            if let Some(value) = self.set_component(cells, &node, index) {
                deq.extend(Self::neighbors(node.coords, value - 1));
            }
        }

        updates
    }

    fn unspread_component(
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
            match self.unset_component(cells, &node, index) {
                Ok(component) => deq.extend(Self::neighbors(node.coords, component - 1)),
                Err(0) => {}
                Err(component) => sources.push((node.coords, component)),
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, component)| self.spread_component(cells, coords, index, component))
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
                let component = self.component(coords, index);
                (component != 0).then(|| self.spread_component(cells, coords, index, component))
            })
            .flatten()
            .collect()
    }

    fn replace_component(&mut self, coords: Point3<f32>, index: usize, value: u8) -> u8 {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        self.block_light_mut(chunk_coords, block_coords)
            .replace_component(index, value)
    }

    fn take_component(&mut self, coords: Point3<f32>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
    }

    fn component(&mut self, coords: Point3<f32>, index: usize) -> u8 {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        self.block_light_mut(chunk_coords, block_coords)
            .component(index)
    }

    fn set_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        node: &LightNode,
        index: usize,
    ) -> Option<u8> {
        let block_light = self.block_light_mut(node.chunk_coords, node.block_coords);
        let component = block_light.component(index);
        let value = node.filtered_value(cells, index);
        (component < value).then(|| {
            block_light.set_component(index, value);
            value
        })
    }

    fn unset_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        node: &LightNode,
        index: usize,
    ) -> Result<u8, u8> {
        let block_light = self.block_light_mut(node.chunk_coords, node.block_coords);
        let component = block_light.component(index);
        if component != 0 && component == node.filtered_value(cells, index) {
            block_light.set_component(index, 0);
            Ok(component)
        } else {
            Err(component)
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
    pub struct BlockLight(u32);
    u8, component, set_component: 3, 0, 6;
}

impl BlockLight {
    const COMPONENT_MAX: u8 = 15;
    const SKYLIGHT_RANGE: Range<usize> = 0..3;
    const TORCHLIGHT_RANGE: Range<usize> = 3..6;

    fn replace_component(&mut self, index: usize, value: u8) -> u8 {
        let component = self.component(index);
        self.set_component(index, value);
        component
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

struct BlockLightSum([(u8, u8); 6]);

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
        self.block_data(cells).light_filter()[index % 3]
    }

    fn block_data(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>) -> &'static BlockData {
        cells
            .get(&self.chunk_coords)
            .map_or(Block::Air, |cell| cell[self.block_coords])
            .data()
    }
}

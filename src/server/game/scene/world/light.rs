use super::{
    block::{Block, BlockArea, BlockData, Side, SIDE_DELTAS},
    chunk::{BlockAction, Chunk, ChunkArea, ChunkCell, Permutation},
};
use crate::server::game::player::Player;
use bitfield::bitfield;
use enum_map::{enum_map, EnumMap};
use nalgebra::{point, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    collections::VecDeque,
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
        let block_light = *self.block_light(coords);
        (0..3)
            .flat_map(|i| self.unspread_channel(cells, coords, i, block_light.channel(i)))
            .collect()
    }

    fn place(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        let block_light = *self.block_light(coords);
        block
            .data()
            .luminance()
            .into_iter()
            .enumerate()
            .flat_map(|(i, v)| {
                self.unspread_channel(cells, coords, i, block_light.channel(i))
                    .into_iter()
                    .chain(self.spread_channel(cells, coords, i, v))
            })
            .collect()
    }

    fn spread_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = VecDeque::from([LightNode::new(coords, value)]);
        let mut updates = FxHashSet::default();
        while let Some(node) = deq.pop_front() {
            if node.set_channel(&mut self.0, index) {
                deq.extend(SIDE_DELTAS.values().map(|delta| {
                    let node = node.visit_next(*delta);
                    updates.insert(node.chunk_coords);
                    node.apply_filter(cells, index)
                }));
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
        // let mut deq = VecDeque::from([LightNode::new(coords, value)]);
        // let mut sources = vec![];
        // let mut updates = FxHashSet::default();
        // while let Some(node) = deq.pop_front() {
        //     if node.unset_channel(&mut self.0, index) {
        //         deq.extend(SIDE_DELTAS.values().filter_map(|delta| {
        //             let node = node.visit_next(*delta)?;
        //             updates.insert(node.chunk_coords);
        //             Some(node)
        //         }));
        //     } else {
        //         sources.push((node.coords, node.value));
        //     }
        // }
        // sources
        //     .into_iter()
        //     .flat_map(|(coords, value)| self.spread_channel(cells, coords, index, value))
        //     .chain(updates)
        //     .collect()
        todo!()
    }

    fn block_light(&mut self, coords: Point3<f32>) -> &BlockLight {
        &self.0.entry(Player::chunk_coords(coords)).or_default()[Player::block_coords(coords)]
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

        for perm in [Permutation([0, 1, 2]), Permutation([0, 2, 1])] {
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
    skylight, set_skylight: 3, 0;
    channel, set_channel: 7, 4, 3;
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 - 1))))
        }))
    }

    pub fn side_lights(&self) -> EnumMap<Side, BlockLight> {
        enum_map! { side => self[SIDE_DELTAS[side]] }
    }
}

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c + 1);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
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
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        Self {
            coords,
            chunk_coords,
            block_coords,
            value,
        }
    }

    fn set_channel(&self, lights: &mut FxHashMap<Point3<i32>, ChunkLight>, index: usize) -> bool {
        let block_light = self.block_light_mut(lights);
        if block_light.channel(index) < self.value {
            block_light.set_channel(index, self.value);
            true
        } else {
            false
        }
    }

    fn unset_channel(&self, lights: &mut FxHashMap<Point3<i32>, ChunkLight>, index: usize) -> bool {
        let block_light = self.block_light_mut(lights);
        if block_light.channel(index) <= self.value {
            block_light.set_channel(index, 0);
            true
        } else {
            false
        }
    }

    fn visit_next(&self, delta: Point3<i8>) -> Self {
        Self::new(
            self.coords + delta.coords.cast(),
            self.value.saturating_sub(1),
        )
    }

    fn apply_filter(mut self, cells: &FxHashMap<Point3<i32>, ChunkCell>, index: usize) -> Self {
        let filter = self.block_data(cells).light_filter()[index];
        self.value = (self.value as f32 * filter).round() as u8;
        self
    }

    fn block_data(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>) -> &'static BlockData {
        cells
            .get(&self.chunk_coords)
            .map_or(Block::Air, |cell| cell[self.block_coords])
            .data()
    }

    fn block_light_mut<'a>(
        &self,
        lights: &'a mut FxHashMap<Point3<i32>, ChunkLight>,
    ) -> &'a mut BlockLight {
        &mut lights.entry(self.chunk_coords).or_default()[self.block_coords]
    }
}

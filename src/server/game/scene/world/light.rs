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
    num::NonZeroU8,
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
            .flat_map(|i| self.unspread_channel(cells, coords, i))
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
            .flat_map(|(i, v)| {
                self.unspread_channel(cells, coords, i)
                    .into_iter()
                    .chain(self.spread_channel(cells, coords, i, Some(v)))
            })
            .collect()
    }

    fn spread_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: Option<u8>,
    ) -> FxHashSet<Point3<i32>> {
        if value == Some(0) {
            return Default::default();
        }

        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        let block_light = self.block_light_mut(chunk_coords, block_coords);
        let channel = block_light.channel(index);
        let value = match value {
            Some(value) if channel < value => {
                block_light.set_channel(index, value);
                value
            }
            None if channel > 0 => channel,
            _ => return Default::default(),
        };

        let mut deq = VecDeque::from([(coords, value - 1)]);
        let mut updates = FxHashSet::default();

        while let Some((coords, value)) = deq.pop_front() {
            deq.extend(
                SIDE_DELTAS
                    .values()
                    .map(|delta| {
                        let coords = coords + delta.coords.cast();
                        let chunk_coords = Player::chunk_coords(coords);
                        let block_coords = Player::block_coords(coords);
                        (coords, chunk_coords, block_coords)
                    })
                    .inspect(|(_, chunk_coords, _)| {
                        updates.insert(*chunk_coords);
                    })
                    .filter_map(|(coords, chunk_coords, block_coords)| {
                        let block_light = self.block_light_mut(chunk_coords, block_coords);
                        let filter = Self::filter(cells, chunk_coords, block_coords, index);
                        let value = Self::filtered(value, filter);
                        if block_light.channel(index) < value {
                            block_light.set_channel(index, value);
                            Some((coords, value - 1))
                        } else {
                            None
                        }
                    }),
            );
        }

        updates
    }

    fn unspread_channel(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        let block_light = self.block_light_mut(chunk_coords, block_coords);
        if let Some(value) = block_light.take_channel(index).checked_sub(1) {
            self.unspread_neighbors(cells, coords, index, value)
        } else {
            self.spread_neighbors(cells, coords, index)
        }
    }

    fn spread_neighbors(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
    ) -> FxHashSet<Point3<i32>> {
        SIDE_DELTAS
            .values()
            .flat_map(|delta| self.spread_channel(cells, coords + delta.coords.cast(), index, None))
            .collect()
    }

    fn unspread_neighbors(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut sources = vec![];
        let mut updates = FxHashSet::default();

        while let Some((coords, expected)) = deq.pop_front() {
            deq.extend(
                SIDE_DELTAS
                    .values()
                    .map(|delta| {
                        let coords = coords + delta.coords.cast();
                        let chunk_coords = Player::chunk_coords(coords);
                        let block_coords = Player::block_coords(coords);
                        (coords, chunk_coords, block_coords)
                    })
                    .inspect(|(_, chunk_coords, _)| {
                        updates.insert(*chunk_coords);
                    })
                    .filter_map(|(coords, chunk_coords, block_coords)| {
                        let block_light = self.block_light_mut(chunk_coords, block_coords);
                        let value = NonZeroU8::new(block_light.take_channel(index))?.get();
                        let filter = Self::filter(cells, chunk_coords, block_coords, index);
                        let expected = Self::filtered(expected, filter);
                        if value == expected {
                            Some((coords, value - 1))
                        } else {
                            sources.push((coords, value));
                            None
                        }
                    }),
            );
        }

        sources
            .into_iter()
            .flat_map(|(coords, value)| self.spread_channel(cells, coords, index, Some(value)))
            .chain(updates)
            .collect()
    }

    fn block_light_mut(
        &mut self,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
    ) -> &mut BlockLight {
        &mut self.0.entry(chunk_coords).or_default()[block_coords]
    }

    fn filter(
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
        index: usize,
    ) -> f32 {
        Self::block_data(cells, chunk_coords, block_coords).light_filter()[index]
    }

    fn filtered(value: u8, filter: f32) -> u8 {
        (value as f32 * filter).round() as u8
    }

    fn block_data(
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        chunk_coords: Point3<i32>,
        block_coords: Point3<u8>,
    ) -> &'static BlockData {
        cells
            .get(&chunk_coords)
            .map_or(Block::Air, |cell| cell[block_coords])
            .data()
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
    skylight, set_skylight: 3, 0;
    channel, set_channel: 7, 4, 3;
}

impl BlockLight {
    fn take_channel(&mut self, index: usize) -> u8 {
        let value = self.channel(index);
        self.set_channel(index, 0);
        value
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 - 1))))
        }))
    }

    pub fn side_lights(&self) -> EnumMap<Side, BlockLight> {
        let light = self[Point3::origin()];
        enum_map! {
            side => {
                let mut side_light = self[SIDE_DELTAS[side]];
                for i in 0..3 {
                    side_light.set_channel(i, side_light.channel(i).max(light.channel(i)));
                }
                side_light
            }
        }
    }
}

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c + 1);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

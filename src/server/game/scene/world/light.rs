use super::{
    block::{Block, BlockArea, BlockData, Side},
    chunk::{BlockAction, Chunk, ChunkArea, ChunkCell},
};
use crate::server::game::{player::Player, scene::world::block::SIDE_DELTAS};
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
        ChunkAreaLight::from_fn(|delta| {
            let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
            let block_coords = delta.map(|c| (c + Chunk::DIM as i8) as u8 % Chunk::DIM as u8);
            self.0
                .get(&chunk_coords)
                .map(|chunk_light| chunk_light[block_coords])
                .unwrap_or_default()
        })
    }

    pub fn apply(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i32>> {
        match action {
            BlockAction::Destroy => Default::default(),
            BlockAction::Place(block) => self.place(cells, coords, *block),
        }
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
            .flat_map(|(i, c)| self.spread_torchlight(cells, coords, i, c))
            .collect()
    }

    fn spread_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::default();
        while let Some((coords, value)) = deq.pop_front() {
            if self.set_torchlight(coords, index, value) {
                if let Some(value) = value.checked_sub(1) {
                    deq.extend(SIDE_DELTAS.values().map(|delta| {
                        let coords = coords + delta.coords.cast();
                        updates.insert(Player::chunk_coords(coords));
                        (coords, Self::filter(cells, coords, index, value))
                    }));
                }
            }
        }
        updates
    }

    fn set_torchlight(&mut self, coords: Point3<f32>, index: usize, value: u8) -> bool {
        let block_light = self.block_light_mut(coords);
        if block_light.torchlight(index) < value {
            block_light.set_torchlight(index, value);
            true
        } else {
            false
        }
    }

    fn filter(
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        index: usize,
        value: u8,
    ) -> u8 {
        (value as f32 * Self::block_data(cells, coords).light_filter()[index]).round() as u8
    }

    fn block_data(
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> &'static BlockData {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        cells
            .get(&chunk_coords)
            .map_or_else(|| Block::Air, |cell| cell[block_coords])
            .data()
    }

    fn block_light_mut(&mut self, coords: Point3<f32>) -> &mut BlockLight {
        let chunk_coords = Player::chunk_coords(coords);
        let block_coords = Player::block_coords(coords);
        &mut self.0.entry(chunk_coords).or_default()[block_coords]
    }
}

#[derive(Default)]
struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

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
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 + BlockArea::RANGE.start)))
            })
        }))
    }

    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        let coords = coords.cast();
        BlockAreaLight::from_fn(|delta| self[coords + delta.coords])
    }
}

impl Index<Point3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c - ChunkArea::RANGE.start);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct BlockLight(u16);
    u8;
    skylight, set_skylight: 3, 0;
    torchlight, set_torchlight: 7, 4, 3;
}

impl Default for BlockLight {
    fn default() -> Self {
        let mut value = BlockLight(0);
        value.set_skylight(15);
        value
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 + BlockArea::RANGE.start)))
            })
        }))
    }

    pub fn side_lights(&self) -> EnumMap<Side, BlockLight> {
        enum_map! { side => self[SIDE_DELTAS[side]] }
    }
}

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c - BlockArea::RANGE.start);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

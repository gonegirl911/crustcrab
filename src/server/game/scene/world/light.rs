use super::{
    block::{Block, BlockArea, BlockData, Side},
    chunk::{BlockAction, Chunk, ChunkArea, ChunkCell},
};
use crate::server::game::{player::Player, scene::world::block::SIDE_DELTAS};
use bitfield::{bitfield, BitRange, BitRangeMut};
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
            BlockAction::Destroy => self.destroy(cells, coords),
            BlockAction::Place(block) => self.place(cells, coords, *block),
        }
    }

    fn place(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        block: Block,
    ) -> FxHashSet<Point3<i32>> {
        let block_data = block.data();
        if block_data.is_glowing() {
            self.spread_torchlight(cells, coords, block_data.luminance().into())
        } else {
            self.unspread_torchlight(cells, coords)
        }
    }

    fn destroy(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> FxHashSet<Point3<i32>> {
        self.unspread_torchlight(cells, coords)
    }

    fn spread_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        torchlight: Torchlight,
    ) -> FxHashSet<Point3<i32>> {
        let mut deq = VecDeque::from([(coords, torchlight)]);
        let mut updates = FxHashSet::default();
        while let Some((coords, torchlight)) = deq.pop_front() {
            if self.set_torchlight(coords, torchlight) {
                let torchlight = torchlight.saturating_component_sub(1);
                deq.extend(SIDE_DELTAS.values().map(|delta| {
                    let coords = coords + delta.coords.cast();
                    updates.insert(Player::chunk_coords(coords));
                    (coords, Self::apply_filter(cells, coords, torchlight))
                }));
            }
        }
        updates
    }

    fn unspread_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
    ) -> FxHashSet<Point3<i32>> {
        todo!()
    }

    fn set_torchlight(&mut self, coords: Point3<f32>, torchlight: Torchlight) -> bool {
        let block_light = self.block_light_mut(coords);
        let prev = block_light.torchlight();
        let curr = prev.component_max(torchlight);
        if prev != curr {
            block_light.set_torchlight(curr);
            true
        } else {
            false
        }
    }

    fn apply_filter(
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<f32>,
        torchlight: Torchlight,
    ) -> Torchlight {
        torchlight.component_mul(Self::block_data(cells, coords).light_filter())
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
    u8, skylight, set_skylight: 3, 0;
    Torchlight, torchlight, set_torchlight: 15, 4;
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

bitfield! {
    #[derive(Clone, Copy, PartialEq, Eq, Default)]
    struct Torchlight(u16);
    u8, channel, set_channel: 3, 0, 3;
}

impl Torchlight {
    fn saturating_component_sub(self, value: u8) -> Self {
        self.map(|c| c.saturating_sub(value))
    }

    fn component_mul(self, values: [f32; 3]) -> Self {
        self.map_with_location(|i, c| (c as f32 * values[i]).round() as u8)
    }

    fn component_max(self, other: Torchlight) -> Self {
        self.zip(other, |a, b| a.max(b))
    }

    fn map<F: FnMut(u8) -> u8>(self, mut f: F) -> Self {
        array::from_fn(|i| f(self.channel(i))).into()
    }

    fn map_with_location<F: FnMut(usize, u8) -> u8>(self, mut f: F) -> Self {
        array::from_fn(|i| f(i, self.channel(i))).into()
    }

    fn zip<F: FnMut(u8, u8) -> u8>(self, other: Torchlight, mut f: F) -> Self {
        array::from_fn(|i| f(self.channel(i), other.channel(i))).into()
    }
}

impl From<[u8; 3]> for Torchlight {
    fn from([r, g, b]: [u8; 3]) -> Self {
        let mut value = Torchlight::default();
        value.set_channel(0, r);
        value.set_channel(1, g);
        value.set_channel(2, b);
        value
    }
}

impl BitRange<Torchlight> for u16 {
    fn bit_range(&self, msb: usize, lsb: usize) -> Torchlight {
        Torchlight(self.bit_range(msb, lsb))
    }
}

impl BitRangeMut<Torchlight> for u16 {
    fn set_bit_range(&mut self, msb: usize, lsb: usize, value: Torchlight) {
        self.set_bit_range(msb, lsb, value.0);
    }
}

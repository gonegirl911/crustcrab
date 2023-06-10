pub mod data;
pub mod light;

use self::data::{
    BlockData, Component, Corner, Side, BLOCK_DATA, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS,
};
use super::action::BlockAction;
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{point, vector, Point3, Vector3};
use serde::Deserialize;
use std::{
    array,
    ops::{Index, IndexMut, Range},
};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Default, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    #[default]
    Air = 0,
    Sand,
    Glowstone,
    GlassMagenta,
    GlassCyan,
}

impl Block {
    pub fn data(self) -> &'static BlockData {
        &BLOCK_DATA[self]
    }

    pub fn apply(&mut self, action: &BlockAction) -> bool {
        match action {
            BlockAction::Place(block) => self.place(*block),
            BlockAction::Destroy => self.destroy(),
        }
    }

    pub fn apply_unchecked(&mut self, action: &BlockAction) {
        match action {
            BlockAction::Place(block) => self.place_unchecked(*block),
            BlockAction::Destroy => self.destroy_unchecked(),
        }
    }

    fn place(&mut self, block: Block) -> bool {
        if *self == Self::Air && block != Self::Air {
            self.place_unchecked(block);
            true
        } else {
            false
        }
    }

    fn place_unchecked(&mut self, block: Block) {
        *self = block;
    }

    fn destroy(&mut self) -> bool {
        if *self != Self::Air {
            self.destroy_unchecked();
            true
        } else {
            false
        }
    }

    fn destroy_unchecked(&mut self) {
        *self = Block::Air;
    }
}

#[derive(Clone, Copy, Default)]
pub struct BlockArea([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl BlockArea {
    pub const DIM: usize = 1 + Self::PADDING * 2;
    pub const PADDING: usize = 1;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..1 + Self::PADDING as i8;

    pub fn new(block: Block) -> Self {
        let mut value = Self::default();
        value[Default::default()] = block;
        value
    }

    pub fn from_fn<F: FnMut(Vector3<i8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(unsafe { Self::delta_unchecked(point![x, y, z]) }))
            })
        }))
    }

    fn visible_sides(self) -> impl Iterator<Item = Side> {
        SIDE_DELTAS
            .into_iter()
            .filter(move |(_, delta)| self.is_visible(*delta))
            .map(|(side, _)| side)
    }

    fn corner_aos(&self, side: Side, is_smoothly_lit: bool) -> EnumMap<Corner, u8> {
        if is_smoothly_lit {
            enum_map! { corner => self.ao(side, corner) }
        } else {
            enum_map! { _ => 3 }
        }
    }

    fn is_visible(&self, delta: Vector3<i8>) -> bool {
        self[delta] != self[Default::default()] && self[delta].data().is_transparent()
    }

    fn ao(&self, side: Side, corner: Corner) -> u8 {
        let components = self.components(side, corner);

        let [edge1, edge2, corner] = [
            components[Component::Edge1],
            components[Component::Edge2],
            components[Component::Corner],
        ];

        if edge1 && edge2 {
            0
        } else {
            3 - (edge1 as u8 + edge2 as u8 + corner as u8)
        }
    }

    fn components(&self, side: Side, corner: Corner) -> EnumMap<Component, bool> {
        SIDE_CORNER_COMPONENT_DELTAS[side][corner].map(|_, delta| self[delta].data().is_opaque())
    }

    pub fn deltas() -> impl Iterator<Item = Vector3<i8>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    unsafe fn delta_unchecked(index: Point3<usize>) -> Vector3<i8> {
        index.coords.map(|c| c as i8 - BlockArea::PADDING as i8)
    }

    unsafe fn index_unchecked(delta: Vector3<i8>) -> Point3<usize> {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl Index<Vector3<i8>> for BlockArea {
    type Output = Block;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &self.0[idx.x][idx.y][idx.z]
    }
}

impl IndexMut<Vector3<i8>> for BlockArea {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &mut self.0[idx.x][idx.y][idx.z]
    }
}

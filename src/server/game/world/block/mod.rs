pub mod data;
pub mod light;

use self::data::{
    BlockData, Component, Corner, Side, BLOCK_DATA, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS,
};
use super::action::BlockAction;
use bitvec::prelude::*;
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{vector, Vector3};
use serde::Deserialize;
use std::ops::Range;

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
pub struct BlockArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in u32));

impl BlockArea {
    pub const DIM: usize = 1 + Self::PADDING * 2;
    pub const PADDING: usize = 1;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..1 + Self::PADDING as i8;

    pub fn from_fn<F: FnMut(Vector3<i8>) -> bool>(mut f: F) -> Self {
        let mut value = Self::default();
        for delta in Self::deltas() {
            value.set(delta, f(delta));
        }
        value
    }

    fn visible_sides(self) -> impl Iterator<Item = Side> {
        SIDE_DELTAS
            .into_iter()
            .filter(move |(_, delta)| self.is_transparent(*delta))
            .map(|(side, _)| side)
    }

    fn corner_aos(self, side: Side, is_smoothly_lit: bool) -> EnumMap<Corner, u8> {
        if is_smoothly_lit {
            enum_map! { corner => self.ao(side, corner) }
        } else {
            enum_map! { _ => 3 }
        }
    }

    fn is_transparent(self, delta: Vector3<i8>) -> bool {
        !self.is_opaque(delta)
    }

    fn is_opaque(self, delta: Vector3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index(delta)) }
    }

    fn set(&mut self, delta: Vector3<i8>, is_opaque: bool) {
        unsafe {
            self.0.set_unchecked(Self::index(delta), is_opaque);
        }
    }

    fn ao(self, side: Side, corner: Corner) -> u8 {
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

    fn components(self, side: Side, corner: Corner) -> EnumMap<Component, bool> {
        SIDE_CORNER_COMPONENT_DELTAS[side][corner].map(|_, delta| self.is_opaque(delta))
    }

    pub fn deltas() -> impl Iterator<Item = Vector3<i8>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    fn index(delta: Vector3<i8>) -> usize {
        assert!(
            Self::AXIS_RANGE.contains(&delta.x)
                && Self::AXIS_RANGE.contains(&delta.y)
                && Self::AXIS_RANGE.contains(&delta.z)
        );
        unsafe { Self::index_unchecked(delta) }
    }

    unsafe fn index_unchecked(delta: Vector3<i8>) -> usize {
        let idx = delta.map(|c| (c + Self::PADDING as i8) as usize);
        idx.x * Self::DIM.pow(2) + idx.y * Self::DIM + idx.z
    }
}

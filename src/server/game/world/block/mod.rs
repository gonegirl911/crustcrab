pub mod area;
pub mod data;

use self::data::{BlockData, BLOCK_DATA};
use super::action::BlockAction;
use crate::shared::color::Rgb;
use bitfield::bitfield;
use enum_map::Enum;
use serde::Deserialize;
use std::{array, ops::Range};

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
        match (*self, action) {
            (Self::Air, BlockAction::Place(Self::Air) | BlockAction::Destroy) => false,
            (Self::Air, BlockAction::Place(block)) => {
                *self = *block;
                true
            }
            (_, BlockAction::Place(_)) => false,
            (_, BlockAction::Destroy) => {
                *self = Self::Air;
                true
            }
        }
    }

    pub fn apply_unchecked(&mut self, action: &BlockAction) {
        *self = match action {
            BlockAction::Place(block) => *block,
            BlockAction::Destroy => Block::Air,
        };
    }
}

bitfield! {
    #[derive(Clone, Copy, PartialEq, Default)]
    pub struct BlockLight(u32);
    pub u8, component, set_component: Self::COMPONENT_MAX.ilog2() as usize, 0, Self::LEN;
}

impl BlockLight {
    const LEN: usize = 6;
    pub const COMPONENT_MAX: u8 = 15;
    pub const SKYLIGHT_RANGE: Range<usize> = 0..3;
    pub const TORCHLIGHT_RANGE: Range<usize> = 3..6;

    pub fn new(skylight: Rgb<u8>, torchlight: Rgb<u8>) -> Self {
        let mut value = Self::default();
        value.set_skylight(skylight);
        value.set_torchlight(torchlight);
        value
    }

    pub fn lum(self) -> f32 {
        (Self::linearize(self.skylight()) + Self::linearize(self.torchlight()))
            .map(|c| c.clamp(0.0, 1.0))
            .lum()
    }

    pub fn map<F: FnMut(u8) -> u8>(self, mut f: F) -> Self {
        array::from_fn(|i| f(self.component(i))).into()
    }

    pub fn zip_map<F: FnMut(u8, u8) -> u8>(self, other: Self, mut f: F) -> Self {
        array::from_fn(|i| f(self.component(i), other.component(i))).into()
    }

    pub fn sup(self, other: Self) -> Self {
        self.zip_map(other, Ord::max)
    }

    fn skylight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(i + Self::SKYLIGHT_RANGE.start))
    }

    fn set_skylight(&mut self, value: Rgb<u8>) {
        for i in Self::SKYLIGHT_RANGE {
            self.set_component(i, value[i % 3]);
        }
    }

    fn torchlight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(i + Self::TORCHLIGHT_RANGE.start))
    }

    fn set_torchlight(&mut self, value: Rgb<u8>) {
        for i in Self::TORCHLIGHT_RANGE {
            self.set_component(i, value[i % 3]);
        }
    }

    fn linearize(color: Rgb<u8>) -> Rgb<f32> {
        color.map(|c| 0.8f32.powi((Self::COMPONENT_MAX - c) as i32))
    }
}

impl From<[u8; Self::LEN]> for BlockLight {
    fn from(components: [u8; Self::LEN]) -> Self {
        let mut value = Self::default();
        for (i, c) in components.into_iter().enumerate() {
            value.set_component(i, c);
        }
        value
    }
}

impl IntoIterator for BlockLight {
    type Item = u8;
    type IntoIter = array::IntoIter<u8, 6>;

    fn into_iter(self) -> Self::IntoIter {
        array::from_fn(|i| self.component(i)).into_iter()
    }
}

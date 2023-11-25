pub mod area;
pub mod data;
pub mod model;

use self::data::{BlockData, BLOCK_DATA};
use super::action::BlockAction;
use crate::shared::{color::Rgb, enum_map::Enum};
use bitfield::bitfield;
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
    DeadBush,
}

impl Block {
    pub fn data(self) -> BlockData {
        BLOCK_DATA[self]
    }

    pub fn apply(&mut self, action: BlockAction) -> bool {
        if self.is_action_valid(action) {
            self.apply_unchecked(action);
            true
        } else {
            false
        }
    }

    pub fn apply_unchecked(&mut self, action: BlockAction) {
        *self = match action {
            BlockAction::Place(block) => block,
            BlockAction::Destroy => Block::Air,
        };
    }

    pub fn is_action_valid(self, action: BlockAction) -> bool {
        match (self, action) {
            (Self::Air, BlockAction::Place(Self::Air) | BlockAction::Destroy) => false,
            (Self::Air, BlockAction::Place(_)) => true,
            (_, BlockAction::Place(_)) => false,
            (_, BlockAction::Destroy) => true,
        }
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

    pub fn lum(self) -> f32 {
        (Self::linearize(self.skylight()) + Self::linearize(self.torchlight()))
            .map(|c| c.clamp(0.0, 1.0))
            .lum()
    }

    pub fn with_component(mut self, index: usize, value: u8) -> Self {
        self.set_component(index, value);
        self
    }

    pub fn sup(self, other: Self) -> Self {
        self.zip_map(other, Ord::max)
    }

    pub fn imap<F: FnMut(usize, u8) -> u8>(self, mut f: F) -> Self {
        array::from_fn(|i| f(i, self.component(i))).into()
    }

    fn zip_map<F: FnMut(u8, u8) -> u8>(self, other: Self, mut f: F) -> Self {
        array::from_fn(|i| f(self.component(i), other.component(i))).into()
    }

    fn skylight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(Self::SKYLIGHT_RANGE.start + i))
    }

    fn torchlight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(Self::TORCHLIGHT_RANGE.start + i))
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

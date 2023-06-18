pub mod area;
pub mod data;

use self::data::{BlockData, BLOCK_DATA};
use super::action::BlockAction;
use crate::shared::color::Rgb;
use bitfield::bitfield;
use enum_map::Enum;
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

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct BlockLight(u32);
    pub u8, component, set_component: 3, 0, 6;
}

impl BlockLight {
    pub const SKYLIGHT_RANGE: Range<usize> = 0..3;
    pub const TORCHLIGHT_RANGE: Range<usize> = 3..6;
    pub const COMPONENT_MAX: u8 = 15;

    pub fn lum(self) -> f32 {
        (Self::linearize(self.skylight()) + Self::linearize(self.torchlight()))
            .map(|c| c.clamp(0.0, 1.0))
            .lum()
    }

    fn skylight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(i + Self::SKYLIGHT_RANGE.start))
    }

    fn torchlight(self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(i + Self::TORCHLIGHT_RANGE.start))
    }

    fn linearize(color: Rgb<u8>) -> Rgb<f32> {
        color.map(|c| 0.8f32.powi((Self::COMPONENT_MAX - c) as i32))
    }
}

impl From<[u8; 6]> for BlockLight {
    fn from(components: [u8; 6]) -> Self {
        let mut value = Self::default();
        for (i, c) in components.into_iter().enumerate() {
            value.set_component(i, c);
        }
        value
    }
}

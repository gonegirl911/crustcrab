pub mod area;
pub mod data;

use self::data::{BlockData, BLOCK_DATA};
use super::action::BlockAction;
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
}

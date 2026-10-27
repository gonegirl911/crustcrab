mod catalog;
pub mod hover;

pub use catalog::Catalog;

use base::color::Rgb;
use bitfield::bitfield;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Block(u8);

bitfield! {
    #[derive(Clone, Copy, Serialize, Deserialize)]
    pub struct Light(pub u32);
    u8, component, set_component: Self::COMPONENT_MAX.ilog2() as usize, 0, Self::LEN;
}

impl Light {
    const LEN: usize = 6;
    pub const COMPONENT_MAX: u8 = 15;
    const SKYLIGHT_RANGE: Range<usize> = 0..3;
    const TORCHLIGHT_RANGE: Range<usize> = 3..6;

    pub fn skylight(&self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(Self::SKYLIGHT_RANGE.start + i))
    }

    pub fn torchlight(&self) -> Rgb<u8> {
        Rgb::from_fn(|i| self.component(Self::TORCHLIGHT_RANGE.start + i))
    }
}

pub mod data;
pub mod light;

use self::data::{BlockData, BLOCK_DATA};
use super::action::BlockAction;
use bitvec::prelude::*;
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{point, vector, Point2, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::ops::Range;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Enum, Deserialize)]
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

    fn place(&mut self, block: Block) -> bool {
        if *self == Self::Air && block != Self::Air {
            *self = block;
            true
        } else {
            false
        }
    }

    fn destroy(&mut self) -> bool {
        if *self != Self::Air {
            *self = Self::Air;
            true
        } else {
            false
        }
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

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Face {
    X = 0,
    Ypos = 1,
    Yneg = 2,
    Z = 3,
}

impl From<Side> for Face {
    fn from(side: Side) -> Self {
        match side {
            Side::Left | Side::Right => Face::X,
            Side::Up => Face::Ypos,
            Side::Down => Face::Yneg,
            Side::Front | Side::Back => Face::Z,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Front,
    Right,
    Back,
    Left,
    Up,
    Down,
}

#[repr(u8)]
#[derive(Clone, Copy, Enum)]
pub enum Corner {
    LowerLeft,
    LowerRight,
    UpperRight,
    UpperLeft,
}

#[repr(u8)]
#[derive(Clone, Copy, Enum)]
enum Component {
    Edge1,
    Edge2,
    Corner,
}

static SIDE_CORNER_SIDES: Lazy<EnumMap<Side, EnumMap<Corner, [Side; 2]>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => enum_map! {
            Corner::LowerLeft => [Side::Left, Side::Down],
            Corner::LowerRight => [Side::Right, Side::Down],
            Corner::UpperRight => [Side::Right, Side::Up],
            Corner::UpperLeft => [Side::Left, Side::Up],
        },
        Side::Right => enum_map! {
            Corner::LowerLeft => [Side::Front, Side::Down],
            Corner::LowerRight => [Side::Back, Side::Down],
            Corner::UpperRight => [Side::Back, Side::Up],
            Corner::UpperLeft => [Side::Front, Side::Up],
        },
        Side::Back => enum_map! {
            Corner::LowerLeft => [Side::Right, Side::Down],
            Corner::LowerRight => [Side::Left, Side::Down],
            Corner::UpperRight => [Side::Left, Side::Up],
            Corner::UpperLeft => [Side::Right, Side::Up],
        },
        Side::Left => enum_map! {
            Corner::LowerLeft => [Side::Back, Side::Down],
            Corner::LowerRight => [Side::Front, Side::Down],
            Corner::UpperRight => [Side::Front, Side::Up],
            Corner::UpperLeft => [Side::Back, Side::Up],
        },
        Side::Up => enum_map! {
            Corner::LowerLeft => [Side::Left, Side::Front],
            Corner::LowerRight => [Side::Right, Side::Front],
            Corner::UpperRight => [Side::Right, Side::Back],
            Corner::UpperLeft => [Side::Left, Side::Back],
        },
        Side::Down => enum_map! {
            Corner::LowerLeft => [Side::Left, Side::Back],
            Corner::LowerRight => [Side::Right, Side::Back],
            Corner::UpperRight => [Side::Right, Side::Front],
            Corner::UpperLeft => [Side::Left, Side::Front],
        },
    }
});

pub static SIDE_DELTAS: Lazy<EnumMap<Side, Vector3<i8>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => -Vector3::z(),
        Side::Right => Vector3::x(),
        Side::Back => Vector3::z(),
        Side::Left => -Vector3::x(),
        Side::Up => Vector3::y(),
        Side::Down => -Vector3::y(),
    }
});

static SIDE_CORNER_DELTAS: Lazy<EnumMap<Side, EnumMap<Corner, Vector3<u8>>>> = Lazy::new(|| {
    SIDE_CORNER_SIDES.map(|s1, corner_sides| {
        corner_sides.map(|_, [s2, s3]| {
            (SIDE_DELTAS[s1] + SIDE_DELTAS[s2] + SIDE_DELTAS[s3]).map(|c| (c + 1) as u8 / 2)
        })
    })
});

#[allow(clippy::type_complexity)]
static SIDE_CORNER_COMPONENT_DELTAS: Lazy<
    EnumMap<Side, EnumMap<Corner, EnumMap<Component, Vector3<i8>>>>,
> = Lazy::new(|| {
    SIDE_CORNER_SIDES.map(|s1, corner_sides| {
        corner_sides.map(|_, [s2, s3]| {
            let delta = SIDE_DELTAS[s1] + SIDE_DELTAS[s2] + SIDE_DELTAS[s3];
            enum_map! {
                Component::Edge1 => delta - SIDE_DELTAS[s3],
                Component::Edge2 => delta - SIDE_DELTAS[s2],
                Component::Corner => delta,
            }
        })
    })
});

static CORNER_TEX_COORDS: Lazy<EnumMap<Corner, Point2<u8>>> = Lazy::new(|| {
    enum_map! {
        Corner::LowerLeft => point![0, 1],
        Corner::LowerRight => point![1, 1],
        Corner::UpperRight => point![1, 0],
        Corner::UpperLeft => point![0, 0],
    }
});

const CORNERS: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::UpperLeft,
];

const FLIPPED_CORNERS: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::LowerLeft,
    Corner::UpperRight,
    Corner::UpperLeft,
];

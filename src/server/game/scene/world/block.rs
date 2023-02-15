use super::light::BlockLight;
use crate::client::game::scene::world::BlockVertex;
use bitvec::prelude::*;
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{point, Point2, Point3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::ops::Range;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Block {
    #[default]
    Air,
    Grass,
    Glowstone,
}

impl Block {
    pub fn vertices(
        self,
        coords: Point3<u8>,
        area: BlockArea,
        light: BlockLight,
    ) -> Option<impl Iterator<Item = BlockVertex>> {
        let data = self.data();
        data.atlas_coords().map(move |side_atlas_coords| {
            area.visible_sides().flat_map(move |side| {
                let corner_vertex_coords = &SIDE_CORNER_VERTEX_COORDS[side];
                let atlas_coords = side_atlas_coords[side];
                let face = side.into();
                let corner_aos = Self::corner_aos(data, side, area);
                Self::indices(corner_aos).into_iter().map(move |corner| {
                    BlockVertex::new(
                        coords + corner_vertex_coords[corner].coords,
                        CORNER_TEX_COORDS[corner],
                        atlas_coords,
                        face,
                        corner_aos[corner],
                        light,
                    )
                })
            })
        })
    }

    pub fn data(self) -> &'static BlockData {
        &BLOCK_DATA[self]
    }

    pub fn is_air(self) -> bool {
        self == Block::Air
    }

    pub fn is_not_air(self) -> bool {
        !self.is_air()
    }

    fn corner_aos(data: &BlockData, side: Side, area: BlockArea) -> EnumMap<Corner, u8> {
        if data.is_not_glowing() {
            enum_map! { corner => Self::ao(side, corner, area) }
        } else {
            enum_map! { _ => 3 }
        }
    }

    fn indices(corner_aos: EnumMap<Corner, u8>) -> [Corner; 6] {
        if corner_aos[Corner::LowerLeft] + corner_aos[Corner::UpperRight]
            > corner_aos[Corner::LowerRight] + corner_aos[Corner::UpperLeft]
        {
            FLIPPED_INDICES
        } else {
            INDICES
        }
    }

    fn ao(side: Side, corner: Corner, area: BlockArea) -> u8 {
        let components = &SIDE_CORNER_COMPONENT_DELTAS[side][corner];

        let [edge1, edge2, corner] = [
            unsafe { area.get_unchecked(components[Component::Edge1]) },
            unsafe { area.get_unchecked(components[Component::Edge2]) },
            unsafe { area.get_unchecked(components[Component::Corner]) },
        ];

        if edge1 && edge2 {
            0
        } else {
            3 - (edge1 as u8 + edge2 as u8 + corner as u8)
        }
    }
}

#[derive(Deserialize)]
pub struct BlockData {
    #[serde(default)]
    atlas_coords: Option<EnumMap<Side, Point2<u8>>>,
    #[serde(default)]
    luminance: Point3<u8>,
    #[serde(default)]
    light_filter: Point3<f32>,
}

impl BlockData {
    fn atlas_coords(&self) -> Option<&EnumMap<Side, Point2<u8>>> {
        self.atlas_coords.as_ref()
    }

    pub fn luminance(&self) -> Point3<u8> {
        self.luminance
    }

    pub fn light_filter(&self) -> Point3<f32> {
        self.light_filter
    }

    pub fn is_transparent(&self) -> bool {
        self.light_filter != point![0.0, 0.0, 0.0]
    }

    pub fn is_opaque(&self) -> bool {
        !self.is_transparent()
    }

    pub fn is_glowing(&self) -> bool {
        self.luminance != point![0, 0, 0]
    }

    pub fn is_not_glowing(&self) -> bool {
        !self.is_glowing()
    }
}

#[derive(Clone, Copy)]
pub struct BlockArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in u32));

impl BlockArea {
    const DIM: usize = (Self::RANGE.end - Self::RANGE.start) as usize;
    const RANGE: Range<i8> = -1..2;

    pub fn from_fn<F: FnMut(Point3<i8>) -> bool>(mut f: F) -> Self {
        let mut data = BitArray::ZERO;
        for x in Self::RANGE {
            for y in Self::RANGE {
                for z in Self::RANGE {
                    let coords = point![x, y, z];
                    unsafe {
                        data.set_unchecked(Self::index_unchecked(coords), f(coords));
                    }
                }
            }
        }
        Self(data)
    }

    fn visible_sides(self) -> impl Iterator<Item = Side> {
        SIDE_DELTAS
            .iter()
            .filter(move |(_, delta)| unsafe { !self.get_unchecked(**delta) })
            .map(|(side, _)| side)
    }

    unsafe fn get_unchecked(&self, coords: Point3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index_unchecked(coords)) }
    }

    unsafe fn index_unchecked(coords: Point3<i8>) -> usize {
        let coords = coords.map(|c| (c - Self::RANGE.start) as usize);
        coords.x * Self::DIM.pow(2) + coords.y * Self::DIM + coords.z
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
enum Side {
    Front,
    Right,
    Back,
    Left,
    Up,
    Down,
}

#[repr(u8)]
#[derive(Clone, Copy, Enum)]
enum Corner {
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

static BLOCK_DATA: Lazy<EnumMap<Block, BlockData>> = Lazy::new(|| {
    toml::from_str(include_str!("../../../../../assets/blocks.toml"))
        .expect("blocks.toml should be valid")
});

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

static SIDE_DELTAS: Lazy<EnumMap<Side, Point3<i8>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => point![0, 0, -1],
        Side::Right => point![1, 0, 0],
        Side::Back => point![0, 0, 1],
        Side::Left => point![-1, 0, 0],
        Side::Up => point![0, 1, 0],
        Side::Down => point![0, -1, 0],
    }
});

static SIDE_CORNER_VERTEX_COORDS: Lazy<EnumMap<Side, EnumMap<Corner, Point3<u8>>>> =
    Lazy::new(|| {
        SIDE_CORNER_SIDES.map(|s1, corner_sides| {
            corner_sides.map(|_, [s2, s3]| {
                (SIDE_DELTAS[s1] + SIDE_DELTAS[s2].coords + SIDE_DELTAS[s3].coords)
                    .map(|c| (c + 1) as u8 / 2)
            })
        })
    });

#[allow(clippy::type_complexity)]
static SIDE_CORNER_COMPONENT_DELTAS: Lazy<
    EnumMap<Side, EnumMap<Corner, EnumMap<Component, Point3<i8>>>>,
> = Lazy::new(|| {
    SIDE_CORNER_SIDES.map(|s1, corner_sides| {
        corner_sides.map(|_, [s2, s3]| {
            let corner = SIDE_DELTAS[s1] + SIDE_DELTAS[s2].coords + SIDE_DELTAS[s3].coords;
            enum_map! {
                Component::Edge1 => corner - SIDE_DELTAS[s3].coords,
                Component::Edge2 => corner - SIDE_DELTAS[s2].coords,
                Component::Corner => corner,
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

const INDICES: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::UpperLeft,
];

const FLIPPED_INDICES: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::LowerLeft,
    Corner::UpperRight,
    Corner::UpperLeft,
];

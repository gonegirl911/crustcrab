use super::{
    area::{BlockArea, BlockAreaLight},
    Block, BlockLight,
};
use crate::{client::game::world::BlockVertex, shared::color::Rgb};
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{point, Point2, Point3, Vector3};
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::{fs, sync::Arc};

pub struct BlockData {
    pub side_tex_indices: Option<EnumMap<Side, u8>>,
    pub luminance: Rgb<u8>,
    pub light_filter: Rgb<f32>,
}

impl BlockData {
    pub fn vertices(
        &self,
        coords: Point3<u8>,
        area: BlockArea,
        area_light: BlockAreaLight,
    ) -> impl Iterator<Item = BlockVertex> + '_ {
        self.side_tex_indices
            .map(move |side_tex_indices| {
                area.visible_sides().flat_map(move |side| {
                    let corner_deltas = SIDE_CORNER_DELTAS[side];
                    let tex_idx = side_tex_indices[side];
                    let face = side.into();
                    let is_smoothly_lit = self.is_smoothly_lit();
                    let corner_aos = area.corner_aos(side, is_smoothly_lit);
                    let corner_lights = area_light.corner_lights(side, area, is_smoothly_lit);
                    Self::corners(corner_aos, corner_lights)
                        .into_iter()
                        .map(move |corner| {
                            BlockVertex::new(
                                coords + corner_deltas[corner],
                                tex_idx,
                                CORNER_TEX_COORDS[corner],
                                face,
                                corner_aos[corner],
                                corner_lights[corner],
                            )
                        })
                })
            })
            .into_iter()
            .flatten()
    }

    pub fn is_glowing(&self) -> bool {
        self.luminance != Default::default()
    }

    pub fn is_transparent(&self) -> bool {
        self.light_filter != Default::default()
    }

    pub fn is_opaque(&self) -> bool {
        !self.is_transparent()
    }

    pub fn is_smoothly_lit(&self) -> bool {
        !self.is_glowing() && self.is_opaque()
    }

    fn corners(
        corner_aos: EnumMap<Corner, u8>,
        corner_lights: EnumMap<Corner, BlockLight>,
    ) -> [Corner; 6] {
        if corner_aos[Corner::LowerLeft] + corner_aos[Corner::UpperRight]
            > corner_aos[Corner::LowerRight] + corner_aos[Corner::UpperLeft]
            || corner_lights[Corner::LowerLeft].lum() + corner_lights[Corner::UpperRight].lum()
                > corner_lights[Corner::LowerRight].lum() + corner_lights[Corner::UpperLeft].lum()
        {
            FLIPPED_CORNERS
        } else {
            CORNERS
        }
    }
}

impl From<RawBlockData> for BlockData {
    fn from(data: RawBlockData) -> Self {
        Self {
            side_tex_indices: data.side_tex_indices(),
            luminance: data.luminance,
            light_filter: data.light_filter,
        }
    }
}

#[derive(Clone, Deserialize)]
struct RawBlockData {
    #[serde(default)]
    side_tex_paths: Option<EnumMap<Side, Arc<String>>>,
    #[serde(default)]
    luminance: Rgb<u8>,
    #[serde(default)]
    light_filter: Rgb<f32>,
}

impl RawBlockData {
    fn side_tex_indices(&self) -> Option<EnumMap<Side, u8>> {
        self.side_tex_paths
            .clone()
            .map(|paths| paths.map(|_, path| TEX_INDICES[&path]))
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Face {
    X = 0,
    YPos = 1,
    YNeg = 2,
    Z = 3,
}

impl From<Side> for Face {
    fn from(side: Side) -> Self {
        match side {
            Side::Left | Side::Right => Face::X,
            Side::Up => Face::YPos,
            Side::Down => Face::YNeg,
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
pub enum Component {
    Edge1,
    Edge2,
    Corner,
}

pub static BLOCK_DATA: Lazy<EnumMap<Block, BlockData>> =
    Lazy::new(|| RAW_BLOCK_DATA.clone().map(|_, data| data.into()));

pub static TEX_PATHS: Lazy<Vec<Arc<String>>> = Lazy::new(|| {
    let mut v = TEX_INDICES.iter().collect::<Vec<_>>();
    v.sort_unstable_by_key(|(_, idx)| *idx);
    v.into_iter().map(|(path, _)| path).cloned().collect()
});

static TEX_INDICES: Lazy<FxHashMap<Arc<String>, u8>> = Lazy::new(|| {
    let mut indices = FxHashMap::default();
    let mut idx = 0;
    RAW_BLOCK_DATA
        .values()
        .filter_map(|data| data.side_tex_paths.as_ref())
        .flat_map(|side_tex_paths| side_tex_paths.values())
        .cloned()
        .for_each(|path| {
            indices.entry(path).or_insert_with(|| {
                let i = idx;
                idx += 1;
                i
            });
        });
    indices
});

static RAW_BLOCK_DATA: Lazy<EnumMap<Block, RawBlockData>> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/blocks.toml").expect("file should exist"))
        .expect("file should be valid")
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
pub static SIDE_CORNER_COMPONENT_DELTAS: Lazy<
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

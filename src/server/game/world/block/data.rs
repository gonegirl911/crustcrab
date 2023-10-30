use super::{
    area::{BlockArea, BlockAreaLight},
    Block, BlockLight,
};
use crate::{client::game::world::BlockVertex, shared::color::Rgb};
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{Point2, Point3, Vector3};
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::{fs, iter, ops::Index, sync::Arc};

#[derive(Clone, Copy)]
pub struct BlockData {
    model: Model,
    tex_indices: Option<TextureData<u8>>,
    pub luminance: Rgb<u8>,
    pub light_filter: Rgb<u8>,
    pub requires_blending: bool,
}

impl BlockData {
    pub fn vertices(
        self,
        coords: Point3<u8>,
        area: BlockArea,
        area_light: BlockAreaLight,
    ) -> impl Iterator<Item = BlockVertex> {
        self.tex_indices
            .map(move |tex_indices| {
                let is_externally_lit = self.is_externally_lit();
                let ao = area.internal_ao();
                let light = area_light.internal_light();
                self.model
                    .side_corner_deltas()
                    .into_iter()
                    .filter_map(move |(side, corner_deltas)| {
                        area.is_side_visible(SIDE_DELTAS[side])
                            .then_some((side, corner_deltas?))
                    })
                    .flat_map(move |(side, corner_deltas)| {
                        let tex_idx = tex_indices[side];
                        let face = side.into();
                        let corner_aos = area.corner_aos(side, is_externally_lit);
                        let corner_lights = area_light.corner_lights(side, area, is_externally_lit);
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
                    .chain(
                        self.model
                            .internal_deltas()
                            .map(move |deltas| {
                                let tex_idx = *tex_indices.internal_texture();
                                deltas.map(move |(corner, delta)| {
                                    BlockVertex::new(
                                        coords + delta,
                                        tex_idx,
                                        CORNER_TEX_COORDS[corner],
                                        Face::YPos,
                                        ao,
                                        light,
                                    )
                                })
                            })
                            .into_iter()
                            .flatten(),
                    )
            })
            .into_iter()
            .flatten()
    }

    fn is_glowing(self) -> bool {
        self.luminance != Default::default()
    }

    pub fn is_transparent(self) -> bool {
        self.light_filter != Default::default() || self.requires_blending
    }

    pub fn is_opaque(self) -> bool {
        !self.is_transparent()
    }

    pub fn is_externally_lit(self) -> bool {
        !self.is_glowing() && self.light_filter == Default::default()
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
            model: data.model,
            tex_indices: data.tex_indices(),
            luminance: data.luminance,
            light_filter: data.light_filter,
            requires_blending: data.requires_blending,
        }
    }
}

#[derive(Clone, Deserialize)]
struct RawBlockData {
    #[serde(default)]
    model: Model,
    #[serde(flatten, default)]
    tex_paths: Option<TextureData<Arc<String>>>,
    #[serde(default)]
    luminance: Rgb<u8>,
    #[serde(default)]
    light_filter: Rgb<u8>,
    #[serde(default)]
    requires_blending: bool,
}

impl RawBlockData {
    fn tex_indices(&self) -> Option<TextureData<u8>> {
        self.tex_paths
            .clone()
            .map(|paths| paths.map(|path| TEX_INDICES[&path]))
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Model {
    #[default]
    Block,
    Flower,
}

impl Model {
    fn side_corner_deltas(self) -> EnumMap<Side, Option<EnumMap<Corner, Vector3<u8>>>> {
        if self == Model::Block {
            enum_map! { side => Some(SIDE_CORNER_DELTAS[side]) }
        } else {
            enum_map! { _ => None }
        }
    }

    fn internal_deltas(self) -> Option<impl Iterator<Item = (Corner, Vector3<u8>)>> {
        (self == Model::Flower).then(std::iter::empty)
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(untagged)]
enum TextureData<T> {
    Single { texture: T },
}

impl<T> TextureData<T> {
    fn textures(&self) -> impl Iterator<Item = &T> {
        match self {
            Self::Single { texture } => iter::once(texture),
        }
    }

    fn internal_texture(&self) -> &T {
        match self {
            Self::Single { texture } => texture,
        }
    }

    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> TextureData<U> {
        match self {
            Self::Single { texture } => TextureData::Single {
                texture: f(texture),
            },
        }
    }
}

impl<T> Index<Side> for TextureData<T> {
    type Output = T;

    fn index(&self, _: Side) -> &Self::Output {
        match self {
            Self::Single { texture } => texture,
        }
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
        .filter_map(|data| data.tex_paths.as_ref())
        .flat_map(TextureData::textures)
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
    toml::from_str(&fs::read_to_string("assets/config/blocks.toml").expect("file should exist"))
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

static CORNER_TEX_COORDS: Lazy<EnumMap<Corner, Point2<u8>>> =
    Lazy::new(|| SIDE_CORNER_DELTAS[Side::Front].map(|_, delta| delta.xy().into()));

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

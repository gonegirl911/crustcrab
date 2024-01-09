use super::{
    area::{BlockArea, BlockAreaLight},
    model::{Model, RawModel},
    Block, BlockLight,
};
use crate::{
    client::game::world::BlockVertex,
    enum_map,
    server::game::world::chunk::Chunk,
    shared::{
        bound::Aabb,
        color::Rgb,
        enum_map::{Enum, EnumMap},
    },
};
use nalgebra::{point, Point2, Point3, Vector3};
use once_cell::sync::Lazy;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::{
    array, fs,
    iter::{self, Zip},
    sync::Arc,
};

#[derive(Clone, Copy)]
pub struct BlockData {
    model: Model,
    pub luminance: Rgb<u8>,
    pub light_filter: Rgb<bool>,
    pub requires_blending: bool,
    pub valid_surface: Option<Block>,
}

impl BlockData {
    pub fn vertices(
        self,
        side: Option<Side>,
        coords: Point3<u8>,
        dims: Point3<u8>,
        tex_dims: Point2<u8>,
        corner_aos: EnumMap<Corner, u8>,
        corner_lights: EnumMap<Corner, BlockLight>,
    ) -> impl Iterator<Item = BlockVertex> {
        let corner_deltas = self.model.corner_deltas(side);
        let face = side.into();
        let corners = Self::corners(corner_aos, corner_lights);
        corner_deltas.iter().flat_map(move |corner_deltas| {
            corners.into_iter().map(move |corner| {
                let tex_coords = CORNER_TEX_COORDS[corner];
                BlockVertex::new(
                    coords + corner_deltas[corner].component_mul(&dims.coords),
                    self.model.tex_index,
                    array::from_fn(|i| tex_coords[i] * tex_dims[i]).into(),
                    face,
                    corner_aos[corner],
                    corner_lights[corner],
                )
            })
        })
    }

    pub fn mesh(
        self,
        coords: Point3<u8>,
        area: BlockArea,
        area_light: &BlockAreaLight,
    ) -> impl Iterator<Item = BlockVertex> + '_ {
        let is_externally_lit = self.is_externally_lit();
        Enum::variants()
            .filter(move |&side| area.is_side_visible(side))
            .flat_map(move |side| {
                self.vertices(
                    side,
                    coords,
                    point![1, 1, 1],
                    point![1, 1],
                    area.corner_aos(side, is_externally_lit),
                    area_light.corner_lights(side, area, is_externally_lit),
                )
            })
    }

    pub fn tex_idx(self) -> u8 {
        self.model.tex_index
    }

    pub fn hitbox(self, coords: Point3<i64>) -> Aabb {
        self.model.hitbox(coords)
    }

    pub fn flat_icon(self) -> Option<impl Iterator<Item = BlockVertex>> {
        let tex_idx = self.model.flat_icon()?;
        let corner_deltas = SIDE_CORNER_DELTAS[Side::Front];
        Some(CORNERS.into_iter().map(move |corner| {
            BlockVertex::new(
                corner_deltas[corner].into(),
                tex_idx,
                CORNER_TEX_COORDS[corner],
                Default::default(),
                Default::default(),
                Default::default(),
            )
        }))
    }

    pub fn is_glowing(self) -> bool {
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
            model: data.model.map_or_else(Default::default, Into::into),
            luminance: data.luminance,
            light_filter: data.light_filter.map(|c| c != 0),
            requires_blending: data.requires_blending,
            valid_surface: data.valid_surface,
        }
    }
}

impl IntoIterator for BlockData {
    type Item = (u8, bool);
    type IntoIter = Zip<<Rgb<u8> as IntoIterator>::IntoIter, <Rgb<bool> as IntoIterator>::IntoIter>;

    fn into_iter(self) -> Self::IntoIter {
        iter::zip(self.luminance, self.light_filter)
    }
}

#[derive(Clone, Deserialize)]
struct RawBlockData {
    #[serde(flatten, default)]
    model: Option<RawModel>,
    #[serde(default)]
    luminance: Rgb<u8>,
    #[serde(default)]
    light_filter: Rgb<u8>,
    #[serde(default)]
    requires_blending: bool,
    #[serde(default)]
    valid_surface: Option<Block>,
}

impl RawBlockData {
    fn tex_path(&self) -> Option<Arc<str>> {
        Some(self.model.as_ref()?.tex_path.clone())
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Face {
    X = 0,
    Top = 1,
    Bottom = 2,
    Z = 3,
}

impl Default for Face {
    fn default() -> Self {
        None::<Side>.into()
    }
}

impl From<Option<Side>> for Face {
    fn from(side: Option<Side>) -> Self {
        match side {
            Some(Side::Left | Side::Right) => Self::X,
            Some(Side::Top) | None => Self::Top,
            Some(Side::Bottom) => Self::Bottom,
            Some(Side::Front | Side::Back) => Self::Z,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Bottom,
    Front,
    Right,
    Back,
    Left,
    Top,
}

impl Side {
    pub fn points(self) -> impl Iterator<Item = (Point3<u8>, Point3<u8>)> {
        let masks = SIDE_MASKS[self];
        (0..Chunk::DIM as u8).flat_map(move |x| {
            (0..Chunk::DIM as u8).map(move |y| {
                let components = [0, Chunk::DIM as u8 - 1, x, y];
                (
                    masks.map(|(i, _)| components[i]),
                    masks.map(|(_, i)| components[i]),
                )
            })
        })
    }
}

#[derive(Clone, Copy, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    LowerLeft,
    LowerRight,
    UpperRight,
    UpperLeft,
}

#[derive(Clone, Copy, Enum)]
pub enum Component {
    Edge1,
    Edge2,
    Corner,
}

pub static BLOCK_DATA: Lazy<EnumMap<Block, BlockData>> =
    Lazy::new(|| RAW_BLOCK_DATA.clone().map(|_, data| data.into()));

pub static TEX_PATHS: Lazy<Vec<Arc<str>>> = Lazy::new(|| {
    let mut paths = Vec::<Arc<str>>::with_capacity(TEX_INDICES.len());
    unsafe {
        for (path, &i) in &*TEX_INDICES {
            paths
                .as_mut_ptr()
                .add(i as usize)
                .write(path.clone().unwrap_or_else(|| "missing_texture.png".into()));
        }
        paths.set_len(paths.capacity());
    }
    paths
});

pub static TEX_INDICES: Lazy<FxHashMap<Option<Arc<str>>, u8>> = Lazy::new(|| {
    let mut indices = FxHashMap::default();
    let mut idx = 0;
    for data in RAW_BLOCK_DATA.values() {
        indices.entry(data.tex_path()).or_insert_with(|| {
            let i = idx;
            idx += 1;
            i
        });
    }
    indices
});

static RAW_BLOCK_DATA: Lazy<EnumMap<Block, RawBlockData>> = Lazy::new(|| {
    toml::from_str(&fs::read_to_string("assets/config/blocks.toml").expect("file should exist"))
        .expect("file should be valid")
});

static SIDE_CORNER_SIDES: Lazy<EnumMap<Side, EnumMap<Corner, [Side; 2]>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => enum_map! {
            Corner::LowerLeft => [Side::Bottom, Side::Left],
            Corner::LowerRight => [Side::Bottom, Side::Right],
            Corner::UpperRight => [Side::Top, Side::Right],
            Corner::UpperLeft => [Side::Top, Side::Left],
        },
        Side::Right => enum_map! {
            Corner::LowerLeft => [Side::Bottom, Side::Front],
            Corner::LowerRight => [Side::Bottom, Side::Back],
            Corner::UpperRight => [Side::Top, Side::Back],
            Corner::UpperLeft => [Side::Top, Side::Front],
        },
        Side::Back => enum_map! {
            Corner::LowerLeft => [Side::Bottom, Side::Right],
            Corner::LowerRight => [Side::Bottom, Side::Left],
            Corner::UpperRight => [Side::Top, Side::Left],
            Corner::UpperLeft => [Side::Top, Side::Right],
        },
        Side::Left => enum_map! {
            Corner::LowerLeft => [Side::Bottom, Side::Back],
            Corner::LowerRight => [Side::Bottom, Side::Front],
            Corner::UpperRight => [Side::Top, Side::Front],
            Corner::UpperLeft => [Side::Top, Side::Back],
        },
        Side::Top => enum_map! {
            Corner::LowerLeft => [Side::Front, Side::Left],
            Corner::LowerRight => [Side::Front, Side::Right],
            Corner::UpperRight => [Side::Back, Side::Right],
            Corner::UpperLeft => [Side::Back, Side::Left],
        },
        Side::Bottom => enum_map! {
            Corner::LowerLeft => [Side::Back, Side::Left],
            Corner::LowerRight => [Side::Back, Side::Right],
            Corner::UpperRight => [Side::Front, Side::Right],
            Corner::UpperLeft => [Side::Front, Side::Left],
        },
    }
});

pub static SIDE_DELTAS: Lazy<EnumMap<Side, Vector3<i8>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => -Vector3::z(),
        Side::Right => Vector3::x(),
        Side::Back => Vector3::z(),
        Side::Left => -Vector3::x(),
        Side::Top => Vector3::y(),
        Side::Bottom => -Vector3::y(),
    }
});

pub static SIDE_MASKS: Lazy<EnumMap<Side, Point3<(usize, usize)>>> = Lazy::new(|| {
    enum_map! {
        Side::Front => point![(2, 2), (3, 3), (0, 1)],
        Side::Right => point![(1, 0), (3, 3), (2, 2)],
        Side::Back => point![(2, 2), (3, 3), (1, 0)],
        Side::Left => point![(0, 1), (3, 3), (2, 2)],
        Side::Top => point![(2, 2), (1, 0), (3, 3)],
        Side::Bottom => point![(2, 2), (0, 1), (3, 3)],
    }
});

static SIDE_CORNER_DELTAS: Lazy<EnumMap<Side, EnumMap<Corner, Vector3<u8>>>> = Lazy::new(|| {
    SIDE_CORNER_SIDES.map(|s1, corner_sides| {
        corner_sides.map(|_, [s2, s3]| {
            (SIDE_DELTAS[s1] + SIDE_DELTAS[s2] + SIDE_DELTAS[s3]).map(|c| c.max(0) as u8)
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
    Lazy::new(|| SIDE_CORNER_DELTAS[Side::Front].map(|_, delta| point![delta.x, 1 - delta.y]));

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

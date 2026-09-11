use super::{
    Block, BlockLight,
    area::{BlockArea, BlockLightArea},
    model::{Model, RawModel},
};
use crate::{
    client::game::world::BlockVertex,
    enum_map,
    server::game::world::chunk::Chunk,
    shared::{
        color::Rgb,
        enum_map::{Enum, EnumMap},
        indexmap::FxIndexSet,
    },
};
use nalgebra::{Point2, Point3, Scalar, Vector3, point};
use rustc_hash::FxHashMap;
use serde::{
    Deserialize, Deserializer,
    de::{self, Unexpected},
};
use std::{array, collections::BTreeMap, fs, ops::Deref, sync::LazyLock};

pub struct BlockData {
    pub model: Model,
    pub luminance: Rgb<u8>,
    pub light_filter: Rgb<bool>,
    pub render_layer: RenderLayer,
    pub valid_surface: Option<Block>,
}

impl BlockData {
    pub fn vertices(
        &self,
        side: Option<Side>,
        coords: Point3<u8>,
        dims: Point3<u8>,
        tex_dims: Point2<u8>,
        corner_aos: EnumMap<Corner, u8>,
        corner_lights: EnumMap<Corner, BlockLight>,
    ) -> impl Iterator<Item = BlockVertex> {
        let corner_deltas = self.model.corner_deltas(side);
        let side_shade = side.into();
        corner_deltas.iter().flat_map(move |corner_deltas| {
            let vertices = enum_map! {
                corner => {
                    let tex_coords = CORNER_TEX_COORDS[corner];
                    BlockVertex::new(
                        coords + corner_deltas[corner].component_mul(&dims.coords),
                        self.model.tex_index,
                        array::from_fn(|i| tex_coords[i] * tex_dims[i]).into(),
                        side_shade,
                        corner_aos[corner],
                        corner_lights[corner],
                    )
                }
            };
            Self::triangulation(&vertices).map(|corner| vertices[corner])
        })
    }

    pub fn mesh(
        &self,
        coords: Point3<u8>,
        area: &BlockArea,
        light_area: &BlockLightArea,
    ) -> impl Iterator<Item = BlockVertex> {
        let is_externally_lit = self.is_externally_lit();
        Enum::variants()
            .filter(|&side| area.is_side_visible(side))
            .flat_map(move |side| {
                self.vertices(
                    side,
                    coords,
                    point![1, 1, 1],
                    point![1, 1],
                    area.corner_aos(side, is_externally_lit),
                    light_area.corner_lights(side, area),
                )
            })
    }

    pub fn flat_icon(&self) -> Option<impl Iterator<Item = BlockVertex>> {
        let tex_idx = self.model.flat_icon()?;
        let corner_deltas = SIDE_CORNER_DELTAS[Side::Front];
        Some(LL_UR_TRIANGULATION.into_iter().map(move |corner| {
            BlockVertex::new(
                corner_deltas[corner].into(),
                tex_idx,
                CORNER_TEX_COORDS[corner],
                SideShade::Top,
                Default::default(),
                Default::default(),
            )
        }))
    }

    pub fn is_glowing(&self) -> bool {
        self.luminance != Default::default()
    }

    pub fn is_opaque(&self) -> bool {
        self.light_filter == Default::default() && self.render_layer == RenderLayer::Opaque
    }

    pub fn is_externally_lit(&self) -> bool {
        !self.is_glowing() && self.light_filter == Default::default()
    }

    fn triangulation(vertices: &EnumMap<Corner, BlockVertex>) -> [Corner; 6] {
        let lower_left = vertices[Corner::LowerLeft].light_factor(0.0).lum();
        let upper_right = vertices[Corner::UpperRight].light_factor(0.0).lum();
        let lower_right = vertices[Corner::LowerRight].light_factor(0.0).lum();
        let upper_left = vertices[Corner::UpperLeft].light_factor(0.0).lum();
        if lower_left + upper_right > lower_right + upper_left {
            LL_UR_TRIANGULATION
        } else {
            LR_UL_TRIANGULATION
        }
    }
}

impl From<RawBlockData<'_>> for BlockData {
    fn from(data: RawBlockData) -> Self {
        Self {
            model: data.model.into(),
            luminance: data.luminance,
            light_filter: data.light_filter,
            render_layer: data.render_layer,
            valid_surface: data.valid_surface.map(|str| STR_TO_BLOCK[str]),
        }
    }
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
struct RawBlockData<'a> {
    #[serde(borrow, flatten)]
    model: RawModel<'a>,
    luminance: Rgb<u8>,
    #[serde(deserialize_with = "RawBlockData::deserialize_light_filter")]
    light_filter: Rgb<bool>,
    render_layer: RenderLayer,
    valid_surface: Option<&'a str>,
}

impl<'a> RawBlockData<'a> {
    fn tex_path(&self) -> &'a str {
        self.model.tex_path
    }

    fn deserialize_light_filter<'de, D>(deserializer: D) -> Result<Rgb<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let filter = Rgb::deserialize(deserializer)?;
        if let Some(c) = filter.into_iter().find(|&c| c > 1) {
            Err(de::Error::invalid_value(
                Unexpected::Unsigned(c),
                &"either 0 or 1",
            ))
        } else {
            Ok(filter.map(|c| c != 0))
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderLayer {
    #[default]
    Opaque,
    Cutout,
    Blended,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideShade {
    X = 0,
    Top = 1,
    Bottom = 2,
    Z = 3,
}

impl From<Option<Side>> for SideShade {
    fn from(side: Option<Side>) -> Self {
        match side {
            Some(Side::Left | Side::Right) => Self::X,
            Some(Side::Top) | None => Self::Top,
            Some(Side::Bottom) => Self::Bottom,
            Some(Side::Front | Side::Back) => Self::Z,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Bottom, // DO NOT MOVE
    Front,
    Right,
    Back,
    Left,
    Top,
}

impl Side {
    #[rustfmt::skip]
    pub fn block_points(self) -> impl Iterator<Item = (Point3<u8>, Point3<u8>)> {
        let axes = SIDE_AXES[self];
        let dim = Chunk::DIM as u8;
        let [normal, neighbor] = if self.is_positive() { [dim - 1, 0] } else { [0, dim - 1] };
        (0..dim).flat_map(move |u| {
            (0..dim).map(move |v| {
                (
                    axes.swizzle(point![normal, u, v]),
                    axes.swizzle(point![neighbor, u, v]),
                )
            })
        })
    }

    pub fn axis(self) -> usize {
        match self {
            Self::Left | Self::Right => 0,
            Self::Top | Self::Bottom => 1,
            Self::Front | Self::Back => 2,
        }
    }

    pub fn is_positive(self) -> bool {
        matches!(self, Self::Back | Self::Right | Self::Top)
    }

    pub fn opp(self) -> Self {
        match self {
            Side::Front => Side::Back,
            Side::Right => Side::Left,
            Side::Back => Side::Front,
            Side::Left => Side::Right,
            Side::Top => Side::Bottom,
            Side::Bottom => Side::Top,
        }
    }
}

#[derive(Clone, Copy, Debug, Enum, Deserialize)]
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

#[derive(Clone, Copy)]
pub struct SideAxes {
    normal: usize,
    u: usize,
    v: usize,
}

impl SideAxes {
    pub fn swizzle<T: Scalar + Default>(&self, coords: Point3<T>) -> Point3<T> {
        let mut swizzled = <[_; _]>::default();
        swizzled[self.normal] = coords.x.clone();
        swizzled[self.u] = coords.y.clone();
        swizzled[self.v] = coords.z.clone();
        swizzled.into()
    }
}

pub(super) static BLOCK_DATA: LazyLock<Box<[BlockData]>> = LazyLock::new(|| {
    let mut data = Box::new_uninit_slice(STR_TO_BLOCK.len());
    for (str, &Block(i)) in &*STR_TO_BLOCK {
        data[i as usize].write(RAW_BLOCK_DATA[str].clone().into());
    }
    unsafe { data.assume_init() }
});

pub static STR_TO_BLOCK: LazyLock<FxHashMap<&str, Block>> = LazyLock::new(|| {
    let mut idx = Block::HARD_CODED_VALUES.len() as u8;
    RAW_BLOCK_DATA
        .keys()
        .map(|&str| {
            if let Some(i) = Block::HARD_CODED_VALUES.iter().position(|&s| s == str) {
                (str, Block(i as u8))
            } else {
                let entry = (str, Block(idx));
                idx += 1;
                entry
            }
        })
        .collect()
});

pub static TEX_PATHS: LazyLock<FxIndexSet<&str>> = LazyLock::new(|| {
    RAW_BLOCK_DATA
        .values()
        .map(RawBlockData::tex_path)
        .collect()
});

static RAW_BLOCK_DATA: LazyLock<BTreeMap<&str, RawBlockData>> = LazyLock::new(|| {
    let path = "assets/config/blocks.toml";
    let contents =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let leaked_contents = Box::leak(contents.into_boxed_str());
    let data = ::toml::from_str::<BTreeMap<_, RawBlockData>>(leaked_contents)
        .unwrap_or_else(|e| panic!("failed to deserialize {path}: {e}"));

    assert!(
        data.len() <= Block::MAX_COUNT,
        "block count must not exceed {}",
        Block::MAX_COUNT,
    );

    if let Some(str) = Block::HARD_CODED_VALUES
        .iter()
        .find(|&str| !data.contains_key(str))
    {
        panic!("\"{str}\" block must be configured");
    }

    if let Some((block, surface)) = data
        .iter()
        .filter_map(|(block, data)| Some((block, data.valid_surface?)))
        .find(|(_, surface)| !data.contains_key(surface))
    {
        panic!(
            "invalid valid_surface \"{surface}\" of block \"{block}\", expected one of [\"{}\"]",
            data.keys()
                .map(Deref::deref)
                .collect::<Vec<_>>()
                .join("\", \""),
        );
    }

    data
});

static SIDE_CORNER_SIDES: LazyLock<EnumMap<Side, EnumMap<Corner, [Side; 2]>>> =
    LazyLock::new(|| {
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

pub static SIDE_DELTAS: LazyLock<EnumMap<Side, Vector3<i8>>> = LazyLock::new(|| {
    enum_map! {
        Side::Front => -Vector3::z(),
        Side::Right => Vector3::x(),
        Side::Back => Vector3::z(),
        Side::Left => -Vector3::x(),
        Side::Top => Vector3::y(),
        Side::Bottom => -Vector3::y(),
    }
});

static SIDE_CORNER_DELTAS: LazyLock<EnumMap<Side, EnumMap<Corner, Vector3<u8>>>> =
    LazyLock::new(|| {
        SIDE_CORNER_SIDES.map(|s1, corner_sides| {
            corner_sides.map(|_, [s2, s3]| {
                (SIDE_DELTAS[s1] + SIDE_DELTAS[s2] + SIDE_DELTAS[s3]).map(|c| c.max(0) as u8)
            })
        })
    });

#[expect(clippy::type_complexity)]
pub static SIDE_CORNER_COMPONENT_DELTAS: LazyLock<
    EnumMap<Side, EnumMap<Corner, EnumMap<Component, Vector3<i8>>>>,
> = LazyLock::new(|| {
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

static CORNER_TEX_COORDS: LazyLock<EnumMap<Corner, Point2<u8>>> =
    LazyLock::new(|| SIDE_CORNER_DELTAS[Side::Front].map(|_, delta| point![delta.x, 1 - delta.y]));

const LL_UR_TRIANGULATION: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::LowerLeft,
    Corner::UpperRight,
    Corner::UpperLeft,
];

const LR_UL_TRIANGULATION: [Corner; 6] = [
    Corner::LowerLeft,
    Corner::LowerRight,
    Corner::UpperLeft,
    Corner::LowerRight,
    Corner::UpperRight,
    Corner::UpperLeft,
];

pub static SIDE_AXES: LazyLock<EnumMap<Side, SideAxes>> = LazyLock::new(|| {
    SIDE_CORNER_SIDES.map(|side, corner_sides| {
        let [lower, left] = corner_sides[Corner::LowerLeft];
        SideAxes {
            normal: side.axis(),
            u: left.axis(),
            v: lower.axis(),
        }
    })
});

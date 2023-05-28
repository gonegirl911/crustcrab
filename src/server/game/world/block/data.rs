use super::{
    light::BlockAreaLight, Block, BlockArea, Corner, Side, CORNERS, CORNER_TEX_COORDS,
    FLIPPED_CORNERS, SIDE_CORNER_DELTAS,
};
use crate::{client::game::world::BlockVertex, shared::color::Rgb};
use enum_map::EnumMap;
use nalgebra::Point3;
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
                    Self::corners(corner_aos).into_iter().map(move |corner| {
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
        self.luminance != Rgb::splat(0)
    }

    fn is_transparent(&self) -> bool {
        self.light_filter != Rgb::splat(0.0)
    }

    pub fn is_opaque(&self) -> bool {
        !self.is_transparent()
    }

    pub fn is_smoothly_lit(&self) -> bool {
        !self.is_glowing() && self.is_opaque()
    }

    fn corners(corner_aos: EnumMap<Corner, u8>) -> [Corner; 6] {
        if corner_aos[Corner::LowerLeft] + corner_aos[Corner::UpperRight]
            > corner_aos[Corner::LowerRight] + corner_aos[Corner::UpperLeft]
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

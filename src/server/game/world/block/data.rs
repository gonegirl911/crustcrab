use super::{Block, Side};
use crate::shared::color::Rgb;
use enum_map::EnumMap;
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
    fn is_glowing(&self) -> bool {
        self.luminance != Rgb::splat(0)
    }

    pub fn is_not_glowing(&self) -> bool {
        !self.is_glowing()
    }

    fn is_transparent(&self) -> bool {
        self.light_filter != Rgb::splat(0.0)
    }

    pub fn is_opaque(&self) -> bool {
        !self.is_transparent()
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
    pub luminance: Rgb<u8>,
    #[serde(default)]
    pub light_filter: Rgb<f32>,
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

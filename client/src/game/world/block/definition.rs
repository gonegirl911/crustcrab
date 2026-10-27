use super::model;
use base::{collections::indexmap::FxIndexSet, toml};
use enum_map::Enum;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::{fs, sync::LazyLock};

pub(super) struct Definition {
    model_index: u8,
    tex_index: u8,
    render_layer: RenderLayer,
}

impl Definition {
    pub(super) const DEFAULT_MODEL_NAME: &str = "cube";
    const DEFAULT_TEX_PATH: &str = "missing_texture.png";
    const MAX_TEX_COUNT: usize = u8::MAX as usize + 1;
}

impl From<Config<'_>> for Definition {
    fn from(
        Config {
            model_name,
            tex_path,
            render_layer,
        }: Config,
    ) -> Self {
        let model_name = model_name.unwrap_or(Self::DEFAULT_MODEL_NAME);
        let tex_path = tex_path.unwrap_or(Self::DEFAULT_TEX_PATH);
        Self {
            model_index: model::DEFINITIONS.get_index_of(model_name).unwrap() as u8,
            tex_index: TEX_PATHS.get_index_of(tex_path).unwrap() as u8,
            render_layer,
        }
    }
}

#[derive(Clone, Deserialize)]
pub(super) struct Config<'a> {
    #[serde(rename = "model")]
    model_name: Option<&'a str>,
    #[serde(rename = "texture")]
    tex_path: Option<&'a str>,
    render_layer: RenderLayer,
}

#[derive(Clone, Copy, Enum, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RenderLayer {
    Opaque,
    Cutout,
    Blended,
}

pub(super) static TEX_PATHS: LazyLock<FxIndexSet<&str>> = LazyLock::new(|| {
    let paths = CONFIGS
        .values()
        .filter_map(|config| config.tex_path)
        .chain([Definition::DEFAULT_TEX_PATH])
        .collect::<FxIndexSet<_>>();

    assert!(
        paths.len() <= Definition::MAX_TEX_COUNT,
        "texture count must not exceed {}",
        Definition::MAX_TEX_COUNT,
    );

    paths
});

pub(super) static CONFIGS: LazyLock<FxHashMap<&str, Config>> = LazyLock::new(|| {
    let path = "client/assets/config/blocks.toml";
    let contents =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let leaked_contents = Box::leak(contents.into_boxed_str());
    toml::from_str(leaked_contents).unwrap_or_else(|e| panic!("failed to deserialize {path}: {e}"))
});

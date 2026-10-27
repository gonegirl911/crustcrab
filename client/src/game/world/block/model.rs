use super::geometry::{Corner, Side};
use crate::game::world::block;
use base::{collections::indexmap::FxIndexMap, toml};
use compact_str::CompactString;
use enum_map::{Enum, EnumMap};
use nalgebra::Vector3;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::sync::LazyLock;
use walkdir::{DirEntry, WalkDir};

pub(super) struct Definition {
    has_flat_icon: bool,
    #[expect(clippy::type_complexity)]
    corner_deltas: EnumMap<Option<Side>, Box<[EnumMap<Corner, Vector3<u8>>]>>,
}

impl From<Config> for Definition {
    fn from(
        Config {
            has_flat_icon,
            side_corner_deltas,
            internal_corner_deltas,
        }: Config,
    ) -> Self {
        let corner_deltas = Enum::variants()
            .zip(
                side_corner_deltas
                    .into_values()
                    .chain([internal_corner_deltas]),
            )
            .collect();
        Self {
            has_flat_icon,
            corner_deltas,
        }
    }
}

#[derive(Clone, Deserialize)]
struct Config {
    has_flat_icon: bool,
    #[expect(clippy::type_complexity)]
    side_corner_deltas: EnumMap<Side, Box<[EnumMap<Corner, Vector3<u8>>]>>,
    internal_corner_deltas: Box<[EnumMap<Corner, Vector3<u8>>]>,
}

pub(super) static DEFINITIONS: LazyLock<FxIndexMap<CompactString, Definition>> =
    LazyLock::new(|| {
        CONFIGS
            .iter()
            .map(|(name, config)| (name.clone(), config.clone().into()))
            .collect()
    });

static CONFIGS: LazyLock<FxHashMap<CompactString, Config>> = LazyLock::new(|| {
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .is_some_and(|s| s.starts_with('.'))
    }

    let configs = WalkDir::new("client/assets/config/models")
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|s| s == "toml"))
        .map(|entry| {
            let path = entry.path();
            (
                path.file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap_or_else(|| panic!("{} should have a valid UTF-8 stem", path.display()))
                    .into(),
                toml::deserialize(path),
            )
        })
        .collect::<FxHashMap<_, _>>();

    assert!(
        configs.contains_key(block::Definition::DEFAULT_MODEL_NAME),
        "\"{}\" model must be configured",
        block::Definition::DEFAULT_MODEL_NAME,
    );

    configs
});

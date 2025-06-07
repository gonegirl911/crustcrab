use crate::{
    server::game::world::block::data::{Corner, Side, TEX_PATHS},
    shared::{
        bound::Aabb,
        enum_map::{Enum, EnumMap},
        toml,
    },
};
use nalgebra::{Point3, Vector3};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer};
use std::{
    iter,
    ops::Deref,
    sync::{Arc, LazyLock},
};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Copy)]
pub struct Model {
    data: &'static ModelData,
    pub tex_index: u8,
}

impl Model {
    pub fn corner_deltas(self, side: Option<Side>) -> &'static CornerDeltas {
        self.data.corner_deltas(side)
    }

    pub fn hitbox(self, coords: Point3<i64>) -> Aabb {
        self.data.hitbox(coords)
    }

    pub fn flat_icon(self) -> Option<u8> {
        self.data.has_flat_icon.then_some(self.tex_index)
    }
}

impl From<RawModel> for Model {
    fn from(model: RawModel) -> Self {
        Self {
            data: &MODEL_DATA[&model.variant],
            tex_index: model.tex_index(),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(from = "RawModelData")]
struct ModelData {
    diagonal: Vector3<f32>,
    has_flat_icon: bool,
    side_corner_deltas: SideCornerDeltas,
}

type SideCornerDeltas = EnumMap<Option<Side>, Box<CornerDeltas>>;

type CornerDeltas = [EnumMap<Corner, Vector3<u8>>];

impl ModelData {
    fn corner_deltas(&self, side: Option<Side>) -> &CornerDeltas {
        &self.side_corner_deltas[side]
    }

    fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        Aabb::new(
            coords.cast() + self.diagonal.map(|c| (1.0 - c) / 2.0),
            self.diagonal,
        )
    }
}

impl From<RawModelData> for ModelData {
    fn from(data: RawModelData) -> Self {
        Self {
            diagonal: data.diagonal,
            has_flat_icon: data.has_flat_icon,
            side_corner_deltas: iter::zip(
                Enum::variants(),
                data.side_corner_deltas
                    .into_values()
                    .chain([data.internal_corner_deltas]),
            )
            .collect(),
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct RawModel {
    #[serde(rename = "model", deserialize_with = "RawModel::deserialize_variant")]
    variant: Arc<str>,
    #[serde(rename = "texture")]
    pub tex_path: Arc<str>,
}

impl RawModel {
    fn tex_index(&self) -> u8 {
        TEX_PATHS
            .get_index_of(&self.tex_path)
            .unwrap_or_else(|| unreachable!()) as u8
    }

    fn deserialize_variant<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let variant = Arc::deserialize(deserializer)?;
        if MODEL_DATA.contains_key(&variant) {
            Ok(variant)
        } else {
            Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&variant),
                &&*format!(
                    "one of \"{}\"",
                    MODEL_DATA
                        .keys()
                        .map(Deref::deref)
                        .collect::<Vec<_>>()
                        .join("\", \""),
                ),
            ))
        }
    }
}

impl Default for RawModel {
    fn default() -> Self {
        static DEFAULT_TEX_PATH: LazyLock<Arc<str>> =
            LazyLock::new(|| "missing_texture.png".into());

        Self {
            variant: DEFAULT_VARIANT.clone(),
            tex_path: DEFAULT_TEX_PATH.clone(),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RawModelData {
    diagonal: Vector3<f32>,
    has_flat_icon: bool,
    side_corner_deltas: EnumMap<Side, Box<CornerDeltas>>,
    internal_corner_deltas: Box<CornerDeltas>,
}

static MODEL_DATA: LazyLock<FxHashMap<Arc<str>, ModelData>> = LazyLock::new(|| {
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .is_some_and(|s| s.starts_with('.'))
    }

    let data = WalkDir::new("assets/config/models")
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|s| s == "toml"))
        .map(|entry| {
            let path = entry.path();
            (
                path.file_stem()
                    .unwrap_or_else(|| unreachable!())
                    .to_str()
                    .unwrap_or_else(|| panic!("{path:?} should have a valid UTF-8 stem"))
                    .into(),
                toml::deserialize(path),
            )
        })
        .collect::<FxHashMap<_, _>>();

    assert!(
        data.contains_key(&*DEFAULT_VARIANT),
        "{} model must be configured",
        *DEFAULT_VARIANT,
    );

    data
});

static DEFAULT_VARIANT: LazyLock<Arc<str>> = LazyLock::new(|| "cube".into());

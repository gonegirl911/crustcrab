use super::{
    data::{Corner, Side, TEX_INDICES},
    Block,
};
use crate::shared::{
    bound::Aabb,
    enum_map::{Enum, EnumMap},
    utils,
};
use nalgebra::{Point3, Vector3};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer};
use std::sync::{Arc, LazyLock};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Copy)]
pub struct Model {
    data: &'static ModelData,
    pub tex_index: u8,
}

impl Model {
    pub fn new(block: Block, model: RawModel) -> Self {
        Self {
            data: &MODEL_DATA[&model.variant(block)],
            tex_index: TEX_INDICES[&model.tex_path],
        }
    }

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

#[derive(Default)]
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

impl<'de> Deserialize<'de> for ModelData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawModelData {
            diagonal: Vector3<f32>,
            #[serde(default)]
            has_flat_icon: bool,
            #[serde(default)]
            side_corner_deltas: EnumMap<Side, Box<CornerDeltas>>,
            #[serde(default)]
            internal_corner_deltas: Box<CornerDeltas>,
        }

        impl RawModelData {
            fn into_side_corner_deltas(self) -> SideCornerDeltas {
                Enum::variants()
                    .zip(
                        self.side_corner_deltas
                            .into_values()
                            .chain([self.internal_corner_deltas]),
                    )
                    .collect()
            }
        }

        impl From<RawModelData> for ModelData {
            fn from(data: RawModelData) -> Self {
                Self {
                    diagonal: data.diagonal,
                    has_flat_icon: data.has_flat_icon,
                    side_corner_deltas: data.into_side_corner_deltas(),
                }
            }
        }

        Ok(RawModelData::deserialize(deserializer)?.into())
    }
}

#[derive(Clone, Deserialize)]
pub struct RawModel {
    #[serde(
        rename = "model",
        deserialize_with = "RawModel::deserialize_variant",
        default
    )]
    variant: Option<Arc<str>>,
    #[serde(rename = "texture", default)]
    pub tex_path: Option<Arc<str>>,
}

impl RawModel {
    fn variant(&self, block: Block) -> Option<Arc<str>> {
        if block != Block::AIR || self.tex_path.is_some() {
            Some(
                self.variant
                    .clone()
                    .unwrap_or_else(|| DEFAULT_VARIANT.clone()),
            )
        } else {
            self.variant.clone()
        }
    }

    fn deserialize_variant<'de, D>(deserializer: D) -> Result<Option<Arc<str>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let variant = Option::<Arc<str>>::deserialize(deserializer)?;
        if !MODEL_DATA.contains_key(&variant) {
            Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&variant.unwrap_or_else(|| unreachable!())),
                &&*format!(
                    "one of \"{}\"",
                    MODEL_DATA
                        .keys()
                        .filter_map(|variant| Some(&**variant.as_ref()?))
                        .collect::<Vec<_>>()
                        .join("\", \"")
                ),
            ))
        } else {
            Ok(variant)
        }
    }
}

static MODEL_DATA: LazyLock<FxHashMap<Option<Arc<str>>, ModelData>> = LazyLock::new(|| {
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
    }

    let data = WalkDir::new("assets/config/models")
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().map_or(false, |s| s == "toml"))
        .map(|entry| {
            let path = entry.path();
            (
                Some(
                    path.file_stem()
                        .unwrap_or_else(|| unreachable!())
                        .to_str()
                        .unwrap_or_else(|| panic!("{path:?} should have a valid UTF-8 stem"))
                        .into(),
                ),
                utils::deserialize(path),
            )
        })
        .chain([Default::default()])
        .collect::<FxHashMap<_, _>>();

    assert!(
        data.contains_key(&Some(DEFAULT_VARIANT.clone())),
        "{} model must be configured",
        *DEFAULT_VARIANT,
    );

    data
});

static DEFAULT_VARIANT: LazyLock<Arc<str>> = LazyLock::new(|| "cube".into());

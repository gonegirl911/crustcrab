use super::data::{Corner, Side, TEX_INDICES};
use crate::{
    enum_map,
    shared::{
        bound::Aabb,
        enum_map::{Display, Enum, EnumMap},
    },
};
use nalgebra::{Point3, Vector3};
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer};
use std::{fs, sync::Arc};

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

impl Default for Model {
    fn default() -> Self {
        Self {
            data: &MODEL_DATA[None],
            tex_index: TEX_INDICES[&None],
        }
    }
}

impl From<RawModel> for Model {
    fn from(model: RawModel) -> Self {
        Self {
            data: &MODEL_DATA[Some(model.variant)],
            tex_index: TEX_INDICES[&Some(model.tex_path)],
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct RawModel {
    #[serde(rename = "model", default)]
    variant: Variant,
    #[serde(rename = "texture")]
    pub tex_path: Arc<str>,
}

#[derive(Clone, Copy, Default, Enum, Display, Deserialize)]
#[display(format = "snek_case")]
#[serde(rename_all = "snake_case")]
enum Variant {
    #[default]
    Cube,
    Flower,
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

static MODEL_DATA: Lazy<EnumMap<Option<Variant>, ModelData>> = Lazy::new(|| {
    enum_map! {
        Some(variant) => {
            let path = format!("assets/config/models/{variant}.toml");
            toml::from_str(&fs::read_to_string(path).expect("file should exist"))
                .expect("file should be valid")
        }
        None => Default::default(),
    }
});

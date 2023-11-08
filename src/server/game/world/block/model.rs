use super::data::{Corner, Side};
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
use std::fs;

#[derive(Clone, Copy, Deserialize)]
pub struct Model<T> {
    #[serde(rename = "model", default)]
    variant: Option<Variant>,
    #[serde(default)]
    pub texture: T,
}

impl<T> Model<T> {
    pub fn corner_deltas(&self, side: Option<Side>) -> Option<&'static [CornerDeltas]> {
        self.data().corner_deltas(side)
    }

    pub fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        self.data().hitbox(coords)
    }

    pub fn flat_icon(&self) -> Option<&T> {
        self.data().has_flat_icon.then_some(&self.texture)
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Model<U> {
        Model {
            variant: self.variant,
            texture: f(self.texture),
        }
    }

    fn data(&self) -> &'static ModelData {
        &MODEL_DATA[self.variant]
    }
}

#[derive(Clone, Copy, PartialEq, Enum, Display, Deserialize)]
#[display(format = "snake_case")]
#[serde(rename_all = "snake_case")]
enum Variant {
    Cube,
    Flower,
}

#[derive(Default, Deserialize)]
struct ModelData {
    diagonal: Vector3<f32>,
    #[serde(default)]
    has_flat_icon: bool,
    #[serde(deserialize_with = "deserialize_side_corner_deltas", flatten)]
    side_corner_deltas: SideCornerDeltas,
}

type SideCornerDeltas = EnumMap<Option<Side>, Option<Vec<CornerDeltas>>>;

type CornerDeltas = EnumMap<Corner, Vector3<u8>>;

impl ModelData {
    fn corner_deltas(&self, side: Option<Side>) -> Option<&[CornerDeltas]> {
        self.side_corner_deltas[side].as_deref()
    }

    fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        Aabb::new(
            coords.cast() + self.diagonal.map(|c| (1.0 - c) / 2.0),
            self.diagonal,
        )
    }
}

fn deserialize_side_corner_deltas<'de, D>(deserializer: D) -> Result<SideCornerDeltas, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct RawSideCornerDeltas {
        #[serde(default)]
        side_corner_deltas: EnumMap<Side, Vec<CornerDeltas>>,
        #[serde(default)]
        internal_corner_deltas: Vec<CornerDeltas>,
    }

    impl From<RawSideCornerDeltas> for SideCornerDeltas {
        fn from(deltas: RawSideCornerDeltas) -> SideCornerDeltas {
            Option::<Side>::variants()
                .zip(
                    deltas
                        .side_corner_deltas
                        .into_values()
                        .chain([deltas.internal_corner_deltas])
                        .map(|deltas| (!deltas.is_empty()).then_some(deltas)),
                )
                .collect()
        }
    }

    Ok(RawSideCornerDeltas::deserialize(deserializer)?.into())
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

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
use std::{fs, iter, ops::Index};

#[derive(Clone, Deserialize)]
pub struct Model<T> {
    #[serde(rename = "model", default)]
    variant: Variant,
    #[serde(flatten)]
    textures: TextureData<T>,
}

impl<T> Model<T> {
    pub fn corner_deltas(&self, side: Option<Side>) -> Option<&'static [CornerDeltas]> {
        self.data().corner_deltas(side)
    }

    pub fn texture(&self, side: Option<Side>) -> &T {
        &self.textures[side]
    }

    pub fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        self.data().hitbox(coords)
    }

    pub fn flat_icon(&self) -> Option<&T> {
        self.data().has_flat_icon.then(|| &self.textures[None])
    }

    pub fn textures(&self) -> impl Iterator<Item = &T> {
        self.textures.textures()
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Model<U> {
        Model {
            variant: self.variant,
            textures: self.textures.map(f),
        }
    }

    fn data(&self) -> &'static ModelData {
        &MODEL_DATA[self.variant]
    }
}

#[derive(Clone, Copy, PartialEq, Default, Enum, Display, Deserialize)]
#[display(format = "snake_case")]
#[serde(rename_all = "snake_case")]
enum Variant {
    #[default]
    Cube,
    Flower,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum TextureData<T> {
    Single { texture: T },
}

impl<T> TextureData<T> {
    fn textures(&self) -> impl Iterator<Item = &T> {
        match self {
            Self::Single { texture } => iter::once(texture),
        }
    }

    fn map<U, F: FnOnce(T) -> U>(self, f: F) -> TextureData<U> {
        match self {
            Self::Single { texture } => TextureData::Single {
                texture: f(texture),
            },
        }
    }
}

impl<T> Index<Option<Side>> for TextureData<T> {
    type Output = T;

    fn index(&self, _: Option<Side>) -> &Self::Output {
        match self {
            Self::Single { texture } => texture,
        }
    }
}

#[derive(Deserialize)]
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

    impl RawSideCornerDeltas {
        fn side_corner_deltas(self) -> SideCornerDeltas {
            Option::<Side>::variants()
                .zip(
                    self.side_corner_deltas
                        .into_values()
                        .chain([self.internal_corner_deltas])
                        .map(|deltas| Some(deltas).filter(|deltas| !deltas.is_empty())),
                )
                .collect()
        }
    }

    Ok(RawSideCornerDeltas::deserialize(deserializer)?.side_corner_deltas())
}

static MODEL_DATA: Lazy<EnumMap<Variant, ModelData>> = Lazy::new(|| {
    enum_map! {
        variant => {
            let path = format!("assets/config/models/{variant}.toml");
            toml::from_str(&fs::read_to_string(path).expect("file should exist"))
                .expect("file should be valid")
        }
    }
});

use super::data::{Corner, Side};
use crate::shared::bound::Aabb;
use enum_map::{enum_map, Enum, EnumMap};
use nalgebra::{Point3, Vector3};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs, iter, ops::Index};
use strum::Display;

#[derive(Clone, Deserialize)]
pub struct Model<T> {
    #[serde(rename = "model", default)]
    variant: Variant,
    #[serde(flatten)]
    textures: TextureData<T>,
}

impl<T> Model<T> {
    pub fn corner_deltas(
        &self,
        side: Option<Side>,
    ) -> impl Iterator<Item = (EnumMap<Corner, Vector3<u8>>, &T)> {
        self.data()
            .corner_deltas(side)
            .map(move |corner_deltas| (corner_deltas, &self.textures[side]))
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

    fn data(&self) -> &ModelData {
        &MODEL_DATA[self.variant]
    }
}

#[derive(Clone, Copy, PartialEq, Default, Display, Enum, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
enum Variant {
    #[default]
    Block,
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
    #[serde(default)]
    side_corner_deltas: EnumMap<Side, Vec<EnumMap<Corner, Vector3<u8>>>>,
    #[serde(default)]
    internal_corner_deltas: Vec<EnumMap<Corner, Vector3<u8>>>,
}

impl ModelData {
    fn corner_deltas(
        &self,
        side: Option<Side>,
    ) -> impl Iterator<Item = EnumMap<Corner, Vector3<u8>>> + '_ {
        if let Some(side) = side {
            &self.side_corner_deltas[side]
        } else {
            &self.internal_corner_deltas
        }
        .iter()
        .copied()
    }

    fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        let offset = self.diagonal.map(|c| (1.0 - c) / 2.0);
        Aabb::new(coords.cast() + offset, self.diagonal)
    }
}

static MODEL_DATA: Lazy<EnumMap<Variant, ModelData>> = Lazy::new(|| {
    enum_map! {
        variant => {
            let path = format!("assets/config/models/{variant}.toml");
            toml::from_str(&fs::read_to_string(&path).expect("file should exist"))
                .expect("file should be valid")
        }
    }
});

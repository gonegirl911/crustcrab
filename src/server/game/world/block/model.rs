use super::data::{Corner, Side};
use crate::shared::bound::Aabb;
use enum_map::EnumMap;
use nalgebra::{vector, Point3, Vector3};
use serde::Deserialize;
use std::slice;

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum Model<T> {
    Block { textures: EnumMap<Side, T> },
    Flower { texture: T },
}

impl<T> Model<T> {
    pub fn corner_deltas(
        &self,
        side: Option<Side>,
    ) -> impl Iterator<Item = (EnumMap<Corner, Vector3<u8>>, &T)> {
        std::iter::empty()
    }

    pub fn flat_icon(&self) -> Option<&T> {
        if let Self::Flower { texture } = self {
            Some(texture)
        } else {
            None
        }
    }

    pub fn hitbox(&self, coords: Point3<i64>) -> Aabb {
        if matches!(self, Self::Flower { .. }) {
            Aabb::new(
                coords.cast() + vector![0.1, 0.0, 0.1],
                vector![0.8, 1.0, 0.8],
            )
        } else {
            Aabb::new(coords.cast(), vector![1.0, 1.0, 1.0])
        }
    }

    pub fn textures(&self) -> impl Iterator<Item = &T> {
        match self {
            Self::Block { textures } => textures.as_slice(),
            Self::Flower { texture } => slice::from_ref(texture),
        }
        .iter()
    }

    pub fn map<U, F: FnMut(T) -> U>(self, mut f: F) -> Model<U> {
        match self {
            Self::Block { textures } => Model::Block {
                textures: textures.map(|_, texture| f(texture)),
            },
            Self::Flower { texture } => Model::Flower {
                texture: f(texture),
            },
        }
    }
}

struct ModelData {
    side_corner_deltas: Option<EnumMap<Side, EnumMap<Corner, Vector3<u8>>>>,
    internal_corner_deltas: Vec<EnumMap<Corner, Vector3<u8>>>,
}

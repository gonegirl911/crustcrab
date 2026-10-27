use crate::position::Position;
use nalgebra::{Matrix4, Point, SVector};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Serialize)]
pub struct Aabb<const D: usize> {
    min: Position<D>,
    max: Position<D>,
}

impl<const D: usize> Aabb<D> {
    pub fn new(origin: Position<D>, diagonal: SVector<f32, D>) -> Self {
        Self::from_corners(origin, origin + diagonal)
    }

    fn from_corners(a: Position<D>, b: Position<D>) -> Self {
        let (min, max) = a.inf_sup(b);
        Self { min, max }
    }

    pub fn relative_to<P: Into<Position<D>>>(self, anchor: P) -> LocalAabb<D> {
        let anchor = anchor.into();
        LocalAabb {
            min: self.min.displacement_from(anchor).into(),
            max: self.max.displacement_from(anchor).into(),
        }
    }
}

impl<'de, const D: usize> Deserialize<'de> for Aabb<D> {
    fn deserialize<Ds>(deserializer: Ds) -> Result<Self, Ds::Error>
    where
        Ds: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw<const D: usize> {
            min: Position<D>,
            max: Position<D>,
        }

        impl<const D: usize> From<Raw<D>> for Aabb<D> {
            fn from(Raw { min, max }: Raw<D>) -> Self {
                Self::from_corners(min, max)
            }
        }

        Raw::deserialize(deserializer).map(Into::into)
    }
}

pub type Aabb3 = Aabb<3>;

pub struct LocalAabb<const D: usize> {
    min: Point<f32, D>,
    max: Point<f32, D>,
}

impl<const D: usize> LocalAabb<D> {
    fn diagonal(&self) -> SVector<f32, D> {
        self.max - self.min
    }
}

impl LocalAabb<3> {
    pub fn to_homogeneous(&self) -> Matrix4<f32> {
        Matrix4::new_translation(&self.min.coords).prepend_nonuniform_scaling(&self.diagonal())
    }
}

pub type LocalAabb3 = LocalAabb<3>;

mod alias;
pub mod coord;
mod swizzle;

pub use alias::*;

use nalgebra::{Point, SVector};
use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    mem,
    ops::{Add, AddAssign, Deref, DerefMut, Sub},
};
use thiserror::Error;

#[derive(Clone, Copy, Serialize)]
pub struct Position<const D: usize> {
    pub coords: Point<i64, D>,
    fract: SVector<f32, D>,
}

impl<const D: usize> Position<D> {
    const FRACT_COMPONENT_MAX: f32 = 1.0f32.next_down();

    fn fract_mut(&mut self) -> FractMut<'_, D> {
        FractMut(self)
    }

    pub(crate) fn inf_sup(&self, other: &Self) -> (Position<D>, Position<D>) {
        let mut inf = *self;
        let mut sup = *other;
        for i in 0..D {
            if (inf.coords[i], inf.fract[i]) > (sup.coords[i], sup.fract[i]) {
                mem::swap(&mut inf.coords[i], &mut sup.coords[i]);
                mem::swap(&mut inf.fract[i], &mut sup.fract[i]);
            }
        }
        (inf, sup)
    }

    pub fn relative_to<P: Into<Self>>(&self, anchor: P) -> Point<f32, D> {
        let anchor = anchor.into();
        let coords = self.coords - anchor.coords;
        let fract = self.fract - anchor.fract;
        (coords.cast() + fract).into()
    }

    pub fn wrapped_into(&self, period: &SVector<i64, D>) -> Point<f32, D> {
        let mut coords = self.coords;
        let fract = &self.fract;
        for i in 0..D {
            coords[i] = coords[i].rem_euclid(period[i]);
        }
        coords.cast() + fract
    }

    fn normalize(&mut self) {
        for i in 0..D {
            let whole = self.fract[i].floor();
            self.coords[i] += whole as i64;
            self.fract[i] = (self.fract[i] - whole).min(Self::FRACT_COMPONENT_MAX);
        }
    }

    #[rustfmt::skip]
    fn checked_normalize(&mut self) -> Result<(), NormalizeError> {
        let mut normalized = *self;
        for i in 0..D {
            let whole = self.fract[i].floor().to_int_checked().ok_or(NormalizeError)?;
            normalized.coords[i] = self.coords[i].checked_add(whole).ok_or(NormalizeError)?;
            normalized.fract[i] = (self.fract[i] - whole as f32).min(Self::FRACT_COMPONENT_MAX);
        }
        *self = normalized;
        Ok(())
    }
}

impl<const D: usize> Default for Position<D> {
    fn default() -> Self {
        Self {
            coords: Point::origin(),
            fract: SVector::zeros(),
        }
    }
}

impl<const D: usize> From<&Position<D>> for Position<D> {
    fn from(position: &Position<D>) -> Self {
        *position
    }
}

impl<const D: usize> From<Point<i64, D>> for Position<D> {
    fn from(coords: Point<i64, D>) -> Self {
        Self {
            coords,
            ..Default::default()
        }
    }
}

impl<'de, const D: usize> Deserialize<'de> for Position<D> {
    fn deserialize<Ds>(deserializer: Ds) -> Result<Self, Ds::Error>
    where
        Ds: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw<const D: usize> {
            coords: Point<i64, D>,
            fract: SVector<f32, D>,
        }

        impl<const D: usize> TryFrom<Raw<D>> for Position<D> {
            type Error = NormalizeError;

            fn try_from(Raw { coords, fract }: Raw<D>) -> Result<Self, Self::Error> {
                let mut value = Self { coords, fract };
                value.checked_normalize()?;
                Ok(value)
            }
        }

        Raw::deserialize(deserializer)?
            .try_into()
            .map_err(de::Error::custom)
    }
}

impl<const D: usize> Add<SVector<f32, D>> for Position<D> {
    type Output = Self;

    fn add(mut self, rhs: SVector<f32, D>) -> Self::Output {
        self += rhs;
        self
    }
}

impl<const D: usize> AddAssign<SVector<f32, D>> for Position<D> {
    fn add_assign(&mut self, rhs: SVector<f32, D>) {
        *self.fract_mut() += rhs;
    }
}

impl<const D: usize> Sub<SVector<i64, D>> for Position<D> {
    type Output = Self;

    fn sub(mut self, rhs: SVector<i64, D>) -> Self::Output {
        self.coords -= rhs;
        self
    }
}

impl<const D: usize> Sub<SVector<f32, D>> for Position<D> {
    type Output = Self;

    fn sub(mut self, rhs: SVector<f32, D>) -> Self::Output {
        *self.fract_mut() -= rhs;
        self
    }
}

struct FractMut<'a, const D: usize>(&'a mut Position<D>);

impl<const D: usize> Deref for FractMut<'_, D> {
    type Target = SVector<f32, D>;

    fn deref(&self) -> &Self::Target {
        &self.0.fract
    }
}

impl<const D: usize> DerefMut for FractMut<'_, D> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0.fract
    }
}

impl<const D: usize> Drop for FractMut<'_, D> {
    fn drop(&mut self) {
        self.0.normalize();
    }
}

#[derive(Debug, Error)]
#[error("position cannot be normalized")]
struct NormalizeError;

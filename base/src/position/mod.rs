mod alias;
pub mod coord;
mod swizzle;

pub use alias::*;

use nalgebra::{Point, SVector};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    mem,
    ops::{Add, AddAssign, Deref, DerefMut, Sub, SubAssign},
};

#[derive(Clone, Copy, Serialize)]
pub struct Position<const D: usize> {
    pub coords: Point<i64, D>,
    fract: SVector<f32, D>,
}

impl<const D: usize> Position<D> {
    fn fract_mut(&mut self) -> FractMut<'_, D> {
        FractMut(self)
    }

    pub fn rem_euclid(mut self, rhs: SVector<i64, D>) -> Self {
        for i in 0..D {
            self.coords[i] = self.coords[i].rem_euclid(rhs[i]);
        }
        self
    }

    pub(crate) fn inf_sup(self, other: Self) -> (Position<D>, Position<D>) {
        let mut inf = self;
        let mut sup = other;
        for i in 0..D {
            if (&inf.coords[i], &inf.fract[i]) > (&sup.coords[i], &sup.fract[i]) {
                mem::swap(&mut inf.coords[i], &mut sup.coords[i]);
                mem::swap(&mut inf.fract[i], &mut sup.fract[i]);
            }
        }
        (inf, sup)
    }

    pub fn displacement_from<P: Into<Self>>(&self, anchor: P) -> SVector<f32, D> {
        let anchor = anchor.into();
        let coords = self.coords - anchor.coords;
        let fract = self.fract - anchor.fract;
        coords.cast() + fract
    }

    fn renormalize(&mut self) {
        let whole = self.fract.map(f32::floor);
        self.coords += whole.map(|c| c as i64);
        self.fract = (self.fract - whole).map(|c| c.min(1.0f32.next_down()));
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

        impl<const D: usize> From<Raw<D>> for Position<D> {
            fn from(Raw { coords, fract }: Raw<D>) -> Self {
                let mut value = Self { coords, fract };
                value.renormalize();
                value
            }
        }

        Raw::deserialize(deserializer).map(Into::into)
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
        self -= rhs;
        self
    }
}

impl<const D: usize> SubAssign<SVector<i64, D>> for Position<D> {
    fn sub_assign(&mut self, rhs: SVector<i64, D>) {
        self.coords -= rhs;
    }
}

impl<const D: usize> Sub<SVector<f32, D>> for Position<D> {
    type Output = Self;

    fn sub(mut self, rhs: SVector<f32, D>) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<const D: usize> SubAssign<SVector<f32, D>> for Position<D> {
    fn sub_assign(&mut self, rhs: SVector<f32, D>) {
        *self.fract_mut() -= rhs;
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
        self.0.renormalize();
    }
}

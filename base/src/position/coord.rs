use super::Position1;
use nalgebra::{point, vector};
use std::ops::{AddAssign, Mul, MulAssign};

#[derive(Clone, Copy, Default)]
pub struct Coord(Position1);

impl Coord {
    pub fn rem_euclid(self, rhs: i64) -> Self {
        Self(self.0.rem_euclid(vector![rhs]))
    }

    pub fn displacement_from(self, anchor: i64) -> f32 {
        self.0.displacement_from(point![anchor])[0]
    }
}

impl From<Position1> for Coord {
    fn from(position: Position1) -> Self {
        Self(position)
    }
}

impl From<i64> for Coord {
    fn from(value: i64) -> Self {
        Self(point![value].into())
    }
}

impl AddAssign<f32> for Coord {
    fn add_assign(&mut self, rhs: f32) {
        self.0 += vector![rhs];
    }
}

impl Mul<f32> for Coord {
    type Output = Self;

    fn mul(mut self, rhs: f32) -> Self::Output {
        self *= rhs;
        self
    }
}

impl MulAssign<f32> for Coord {
    fn mul_assign(&mut self, rhs: f32) {
        *self.0.fract_mut() *= rhs;
    }
}

use super::Position1;
use nalgebra::{point, vector};
use std::ops::{AddAssign, Mul};

#[derive(Clone, Copy, Default)]
pub struct Coord(Position1);

impl Coord {
    pub fn relative_to<C: Into<Self>>(self, anchor: C) -> f32 {
        self.0.relative_to(anchor.into().0)[0]
    }

    pub fn wrapped_into(self, period: i64) -> f32 {
        self.0.wrapped_into(&vector![period])[0]
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
        *self.0.fract_mut() *= rhs;
        self
    }
}

use nalgebra::{Point, SVector};
use std::{
    iter,
    ops::{Add, Mul},
};

pub fn lerp<T: Lerp>(a: T, b: T, t: f32) -> T {
    a.lerp(b, t)
}

pub trait Lerp {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl<T> Lerp for T
where
    T: Add<Output = Self> + Mul<f32, Output = Self>,
{
    fn lerp(self, other: Self, t: f32) -> Self {
        self * (1.0 - t) + other * t
    }
}

pub fn inv_lerp(a: f32, b: f32, value: f32) -> f32 {
    (value - a) / (b - a)
}

pub fn ramp(start: f32, end: f32, value: f32) -> f32 {
    inv_lerp(start, end, value).clamp(0.0, 1.0)
}

// ------------------------------------------------------------------------------------------------

pub fn magnitude_squared<const N: usize>(vector: SVector<i32, N>) -> u128 {
    vector.map(|c| (c.unsigned_abs() as u128).pow(2)).sum()
}

pub fn distance_squared<const N: usize>(a: Point<i32, N>, b: Point<i32, N>) -> u128 {
    iter::zip(&a.coords, &b.coords)
        .map(|(a, &b)| (a.abs_diff(b) as u128).pow(2))
        .sum()
}

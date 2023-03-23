use bytemuck::{Pod, Zeroable};
use nalgebra::Point3;
use serde::Deserialize;
use std::{
    array,
    ops::{Add, Index, Mul, Neg, Sub},
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Default, Zeroable, Pod, Deserialize)]
pub struct Rgb<T>([T; 3]);

impl<T> Rgb<T> {
    pub fn new(r: T, g: T, b: T) -> Self {
        Self([r, g, b])
    }
}

impl<T: Copy> Rgb<T> {
    pub fn splat(v: T) -> Self {
        Self([v; 3])
    }
}

impl Rgb<f32> {
    pub fn exp(self) -> Self {
        Self(self.0.map(f32::exp))
    }
}

impl<T> Index<usize> for Rgb<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T: Add + Copy> Add for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn add(self, rhs: Self) -> Self::Output {
        Rgb(array::from_fn(|i| self[i] + rhs[i]))
    }
}

impl<T: Sub + Copy> Sub for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn sub(self, rhs: Self) -> Self::Output {
        Rgb(array::from_fn(|i| self[i] - rhs[i]))
    }
}

impl<T: Mul + Copy> Mul for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: Self) -> Self::Output {
        Rgb(array::from_fn(|i| self[i] * rhs[i]))
    }
}

impl<T: Mul + Copy> Mul<T> for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: T) -> Self::Output {
        Rgb(self.0.map(|c| c * rhs))
    }
}

impl<T: Neg> Neg for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn neg(self) -> Self::Output {
        Rgb(self.0.map(Neg::neg))
    }
}

impl<T> IntoIterator for Rgb<T> {
    type Item = T;
    type IntoIter = array::IntoIter<T, 3>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default, Zeroable, Pod)]
pub struct Float3 {
    data: [f32; 3],
    padding: f32,
}

impl From<Rgb<f32>> for Float3 {
    fn from(rgb: Rgb<f32>) -> Self {
        Self {
            data: rgb.0,
            ..Default::default()
        }
    }
}

impl From<Point3<f32>> for Float3 {
    fn from(point: Point3<f32>) -> Self {
        Self {
            data: point.into(),
            ..Default::default()
        }
    }
}

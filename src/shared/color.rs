use bytemuck::{Pod, Zeroable};
use nalgebra::Vector3;
use serde::Deserialize;
use std::{
    array,
    iter::Sum,
    ops::{Add, Index, Mul},
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Default, Zeroable, Pod, Deserialize)]
pub struct Rgb<T>([T; 3]);

impl<T> Rgb<T> {
    pub const fn new(r: T, g: T, b: T) -> Self {
        Self([r, g, b])
    }

    pub fn from_fn<F: FnMut(usize) -> T>(f: F) -> Self {
        Self(array::from_fn(f))
    }

    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Rgb<U> {
        Rgb(self.0.map(f))
    }

    pub fn zip_map<U, V, F>(self, other: Rgb<U>, mut f: F) -> Rgb<V>
    where
        T: Copy,
        U: Copy,
        F: FnMut(T, U) -> V,
    {
        Rgb::from_fn(|i| f(self[i], other[i]))
    }

    fn sum<S: Sum<T>>(self) -> S {
        self.into_iter().sum()
    }
}

impl<T: Copy> Rgb<T> {
    pub const fn splat(v: T) -> Self {
        Self([v; 3])
    }
}

impl<T: Mul + Copy> Rgb<T> {
    fn dot<S: Sum<T::Output>>(self, other: Rgb<T>) -> S {
        (self * other).sum()
    }
}

impl Rgb<f32> {
    pub fn lum(self) -> f32 {
        self.dot(Rgb::new(0.2126, 0.7152, 0.0722))
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
        self.zip_map(rhs, Add::add)
    }
}

impl<T: Mul + Copy> Mul for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.zip_map(rhs, Mul::mul)
    }
}

impl<T: Mul + Copy> Mul<T> for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: T) -> Self::Output {
        self.map(|c| c * rhs)
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

impl From<Vector3<f32>> for Float3 {
    fn from(vector: Vector3<f32>) -> Self {
        Self {
            data: vector.into(),
            ..Default::default()
        }
    }
}

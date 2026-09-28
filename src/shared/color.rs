use bytemuck::{Pod, Zeroable};
use nalgebra::{Point3, Vector3};
use serde::Deserialize;
use std::{
    array,
    iter::Sum,
    ops::{Add, Index, Mul},
};

#[derive(Clone, Copy, PartialEq, Default, Zeroable, Pod, Deserialize)]
#[repr(transparent)]
pub struct Rgb<T>([T; 3]);

impl<T> Rgb<T> {
    fn new(r: T, g: T, b: T) -> Self {
        Self([r, g, b])
    }

    pub fn from_fn<F: FnMut(usize) -> T>(f: F) -> Self {
        Self(array::from_fn(f))
    }

    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Rgb<U> {
        Rgb(self.0.map(f))
    }

    fn zip_map<U, V, F>(self, other: Rgb<U>, mut f: F) -> Rgb<V>
    where
        T: Copy,
        U: Copy,
        F: FnMut(T, U) -> V,
    {
        Rgb::from_fn(|i| f(self[i], other[i]))
    }

    pub fn with_alpha(self, a: T) -> Rgba<T> {
        let [r, g, b] = self.0;
        Rgba([r, g, b, a])
    }
}

impl<T: Copy> Rgb<T> {
    pub fn splat(value: T) -> Self {
        Self([value; 3])
    }
}

impl<T: Mul + Copy> Rgb<T> {
    fn dot<S: Sum<T::Output>>(self, other: Self) -> S {
        (self * other).into_iter().sum()
    }
}

impl Rgb<f32> {
    pub fn lum(&self) -> f32 {
        self.dot(Self::new(0.2126, 0.7152, 0.0722))
    }

    pub fn saturate(self) -> Self {
        self.map(|c| c.clamp(0.0, 1.0))
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

impl<T: Mul + Copy> Mul<T> for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: T) -> Self::Output {
        self.map(|c| c * rhs)
    }
}

impl<T: Mul + Copy> Mul for Rgb<T> {
    type Output = Rgb<T::Output>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.zip_map(rhs, Mul::mul)
    }
}

impl<T> IntoIterator for Rgb<T> {
    type Item = T;
    type IntoIter = array::IntoIter<T, 3>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(transparent)]
pub struct Rgba<T>([T; 4]);

impl<T> Rgba<T> {
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Rgba<U> {
        Rgba(self.0.map(f))
    }
}

impl<T> Index<usize> for Rgba<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Clone, Copy, Default, Zeroable, Pod)]
#[repr(C, align(16))]
pub struct Float3 {
    data: [f32; 3],
    padding: f32,
}

impl From<Vector3<f32>> for Float3 {
    fn from(vector: Vector3<f32>) -> Self {
        Self {
            data: vector.into(),
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

impl From<Rgb<f32>> for Float3 {
    fn from(color: Rgb<f32>) -> Self {
        Self {
            data: color.0,
            ..Default::default()
        }
    }
}

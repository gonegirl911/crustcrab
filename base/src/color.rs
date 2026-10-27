use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use std::{
    array,
    ops::{Add, Index, Mul},
};

#[derive(Clone, Copy, Zeroable, Pod, Deserialize)]
#[repr(transparent)]
pub struct Rgb<T>([T; 3]);

impl<T> Rgb<T> {
    pub fn from_fn<F: FnMut(usize) -> T>(f: F) -> Self {
        Self(array::from_fn(f))
    }

    fn map<U, F: FnMut(T) -> U>(self, f: F) -> Rgb<U> {
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

impl Rgb<f32> {
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

impl<T> From<Rgb<T>> for [T; 3] {
    fn from(rgb: Rgb<T>) -> Self {
        rgb.0
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

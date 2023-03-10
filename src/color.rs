use bytemuck::{Pod, Zeroable};
use serde::Deserialize;
use std::{array, ops::Index};

#[derive(Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub struct Rgb<T>([T; 3]);

impl<T> Rgb<T> {
    pub const fn new(r: T, g: T, b: T) -> Self {
        Self([r, g, b])
    }
}

impl<T: Copy> Rgb<T> {
    pub const fn splat(v: T) -> Self {
        Self([v; 3])
    }
}

impl<T> Index<usize> for Rgb<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T> IntoIterator for Rgb<T> {
    type Item = T;
    type IntoIter = array::IntoIter<T, 3>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Rgb<f32>> for wgpu::Color {
    fn from(Rgb([r, g, b]): Rgb<f32>) -> Self {
        Self {
            r: r.into(),
            g: g.into(),
            b: b.into(),
            a: 1.0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Float3 {
    data: [f32; 3],
    padding: f32,
}

impl From<Rgb<f32>> for Float3 {
    fn from(rgb: Rgb<f32>) -> Self {
        Self {
            data: rgb.0,
            padding: 0.0,
        }
    }
}

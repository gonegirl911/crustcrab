use crate::server::game::world::chunk::Chunk;
use nalgebra::{Point, SVector, Scalar};
use serde::de::DeserializeOwned;
use std::{
    fs, iter,
    ops::{Add, Mul},
    path::Path,
};

pub fn deserialize<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> T {
    let path = path.as_ref();
    let contents = fs::read_to_string(path);
    toml::from_str(&contents.unwrap_or_else(|e| panic!("failed to open {path:?}: {e}")))
        .unwrap_or_else(|e| panic!("failed to deserialize {path:?}: {e}"))
}

pub fn lerp<T: Lerp>(a: T, b: T, t: f32) -> T {
    a * (1.0 - t) + b * t
}

pub fn chunk_coords<T: WorldCoords>(t: T) -> T::Point<i32> {
    t.chunk_coords()
}

pub fn block_coords<T: WorldCoords>(t: T) -> T::Point<u8> {
    t.block_coords()
}

pub fn coords<T: WorldCoords>(t: T) -> T::Point<i64> {
    t.coords()
}

pub fn magnitude_squared<const N: usize>(a: Point<i32, N>, b: Point<i32, N>) -> u128 {
    iter::zip(&a.coords, &b.coords)
        .map(|(a, &b)| (a.abs_diff(b) as u128).pow(2))
        .sum()
}

pub trait Lerp: Mul<f32, Output = Self> + Add<Output = Self> + Sized {}

impl<T: Mul<f32, Output = T> + Add<Output = T>> Lerp for T {}

pub trait WorldCoords {
    type Point<T: Scalar>;

    fn chunk_coords(self) -> Self::Point<i32>;
    fn block_coords(self) -> Self::Point<u8>;
    fn coords(self) -> Self::Point<i64>;
}

impl<const D: usize> WorldCoords for (SVector<i32, D>, SVector<u8, D>) {
    type Point<T: Scalar> = SVector<T, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.0
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.1
    }

    fn coords(self) -> Self::Point<i64> {
        self.0.cast() * Chunk::DIM as i64 + self.1.cast()
    }
}

impl<const D: usize> WorldCoords for (Point<i32, D>, Point<u8, D>) {
    type Point<T: Scalar> = Point<T, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.0
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.1
    }

    fn coords(self) -> Self::Point<i64> {
        coords((self.0.coords, self.1.coords)).into()
    }
}

impl<const D: usize> WorldCoords for Point<i64, D> {
    type Point<U: Scalar> = Point<U, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| div_floor(c, Chunk::DIM as i64) as i32)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }

    fn coords(self) -> Self::Point<i64> {
        self
    }
}

impl<const D: usize> WorldCoords for Point<f32, D> {
    type Point<U: Scalar> = Point<U, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| (c / Chunk::DIM as f32).floor() as i32)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as f32) as u8)
    }

    fn coords(self) -> Self::Point<i64> {
        self.map(|c| c as i64)
    }
}

fn div_floor(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if (r > 0 && b < 0) || (r < 0 && b > 0) {
        d - 1
    } else {
        d
    }
}

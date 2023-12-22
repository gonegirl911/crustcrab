use crate::server::game::world::chunk::Chunk;
use nalgebra::{Point, SVector, Scalar};
use std::{
    iter::Sum,
    ops::{Add, Mul},
};

pub fn lerp<T: Lerp>(a: T, b: T, t: f32) -> T {
    a.lerp(b, t)
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

pub fn magnitude_squared<T, const N: usize>(vector: SVector<T, N>) -> T
where
    T: Copy + Mul<Output = T> + Sum<T>,
{
    vector.iter().copied().map(|c| c * c).sum()
}

pub trait Lerp {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl<T: Mul<f32, Output = T> + Add<Output = T>> Lerp for T {
    fn lerp(self, other: Self, t: f32) -> Self {
        self * (1.0 - t) + other * t
    }
}

pub trait WorldCoords {
    type Point<T: Scalar>;

    fn chunk_coords(self) -> Self::Point<i32>;
    fn block_coords(self) -> Self::Point<u8>;
    fn coords(self) -> Self::Point<i64>;
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
        self.0.cast() * Chunk::DIM as i64 + self.1.coords.cast()
    }
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

impl WorldCoords for (i32, u8) {
    type Point<T: Scalar> = T;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.0
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.1
    }

    fn coords(self) -> Self::Point<i64> {
        self.0 as i64 * Chunk::DIM as i64 + self.1 as i64
    }
}

impl<const D: usize> WorldCoords for Point<i64, D> {
    type Point<U: Scalar> = Point<U, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(chunk_coords)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(block_coords)
    }

    fn coords(self) -> Self::Point<i64> {
        self
    }
}

impl<const D: usize> WorldCoords for Point<f32, D> {
    type Point<U: Scalar> = Point<U, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(chunk_coords)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(block_coords)
    }

    fn coords(self) -> Self::Point<i64> {
        self.map(coords)
    }
}

impl WorldCoords for i64 {
    type Point<T: Scalar> = T;

    fn chunk_coords(self) -> Self::Point<i32> {
        div_floor(self, Chunk::DIM as i64) as i32
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.rem_euclid(Chunk::DIM as i64) as u8
    }

    fn coords(self) -> Self::Point<i64> {
        self
    }
}

impl WorldCoords for f32 {
    type Point<T: Scalar> = T;

    fn chunk_coords(self) -> Self::Point<i32> {
        (self / Chunk::DIM as f32).floor() as i32
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.rem_euclid(Chunk::DIM as f32) as u8
    }

    fn coords(self) -> Self::Point<i64> {
        self as i64
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

use crate::server::game::world::chunk::Chunk;
use nalgebra::{Point, SVector, Scalar};

pub fn chunk_coords<T: IntoWorldCoords>(t: T) -> T::Point<i32> {
    t.into_chunk_coords()
}

pub fn block_coords<T: IntoWorldCoords>(t: T) -> T::Point<u8> {
    t.into_block_coords()
}

pub fn coords<T: IntoWorldCoords>(t: T) -> T::Point<i64> {
    t.into_coords()
}

pub fn magnitude_squared<T: MagnitudeSquared>(t: T) -> T::Output {
    t.magnitude_squared()
}

pub fn div_floor(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if (r > 0 && b < 0) || (r < 0 && b > 0) {
        d - 1
    } else {
        d
    }
}

pub const fn div_ceil(a: usize, b: usize) -> usize {
    let d = a / b;
    let r = a % b;
    if r > 0 && b > 0 {
        d + 1
    } else {
        d
    }
}

pub trait IntoWorldCoords {
    type Point<T: Scalar>;

    fn into_chunk_coords(self) -> Self::Point<i32>;
    fn into_block_coords(self) -> Self::Point<u8>;
    fn into_coords(self) -> Self::Point<i64>;
}

impl<const D: usize> IntoWorldCoords for (Point<i32, D>, Point<u8, D>) {
    type Point<T: Scalar> = Point<T, D>;

    fn into_chunk_coords(self) -> Self::Point<i32> {
        self.0
    }

    fn into_block_coords(self) -> Self::Point<u8> {
        self.1
    }

    fn into_coords(self) -> Self::Point<i64> {
        self.0.cast() * Chunk::DIM as i64 + self.1.coords.cast()
    }
}

impl<const D: usize> IntoWorldCoords for Point<i64, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn into_chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| div_floor(c, Chunk::DIM as i64) as i32)
    }

    fn into_block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }

    fn into_coords(self) -> Self::Point<i64> {
        self
    }
}

impl<const D: usize> IntoWorldCoords for Point<f32, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn into_chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| (c / Chunk::DIM as f32).floor() as i32)
    }

    fn into_block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as f32) as u8)
    }

    fn into_coords(self) -> Self::Point<i64> {
        self.map(|c| c as i64)
    }
}

pub trait MagnitudeSquared {
    type Output;

    fn magnitude_squared(self) -> Self::Output;
}

impl<const D: usize> MagnitudeSquared for SVector<i32, D> {
    type Output = u32;

    fn magnitude_squared(self) -> Self::Output {
        self.map(|c| c.pow(2)).sum() as u32
    }
}

impl<const D: usize> MagnitudeSquared for SVector<i64, D> {
    type Output = u64;

    fn magnitude_squared(self) -> Self::Output {
        self.map(|c| c.pow(2)).sum() as u64
    }
}

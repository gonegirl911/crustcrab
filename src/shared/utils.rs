use crate::server::game::world::chunk::Chunk;
use nalgebra::{Point, SVector, Scalar};
use rayon::prelude::*;
use serde::de::DeserializeOwned;
use std::{
    collections::linked_list,
    fs,
    iter::{self, Flatten},
    ops::{Add, Mul},
    path::Path,
};

pub fn deserialize<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> T {
    let path = path.as_ref();
    let contents = fs::read_to_string(path);
    toml::from_str(&contents.unwrap_or_else(|e| panic!("failed to open {path:?}: {e}")))
        .unwrap_or_else(|e| panic!("failed to deserialize {path:?}: {e}"))
}

// ------------------------------------------------------------------------------------------------

pub fn lerp<T: Lerp>(a: T, b: T, t: f32) -> T {
    a * (1.0 - t) + b * t
}

pub trait Lerp = Mul<f32, Output = Self> + Add<Output = Self> + Sized;

// ------------------------------------------------------------------------------------------------

pub fn magnitude_squared<const N: usize>(a: Point<i32, N>, b: Point<i32, N>) -> u128 {
    iter::zip(&a.coords, &b.coords)
        .map(|(a, &b)| (a.abs_diff(b) as u128).pow(2))
        .sum()
}

// ------------------------------------------------------------------------------------------------

pub fn chunk_coords<T: WorldCoords>(t: T) -> T::Point<i32> {
    t.chunk_coords()
}

pub fn block_coords<T: WorldCoords>(t: T) -> T::Point<u8> {
    t.block_coords()
}

pub fn coords<T: WorldCoords>(t: T) -> T::Point<i64> {
    t.coords()
}

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
        self.map(|c| c.div_floor(Chunk::DIM as i64) as i32)
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

// ------------------------------------------------------------------------------------------------

pub trait IntoSequentialIteratorExt {
    type Item;
    type IntoSeqIter: Iterator<Item = Self::Item>;

    fn into_seq_iter(self) -> Self::IntoSeqIter;
}

impl<I: ParallelIterator> IntoSequentialIteratorExt for I {
    type Item = I::Item;
    type IntoSeqIter = Flatten<linked_list::IntoIter<Vec<Self::Item>>>;

    fn into_seq_iter(self) -> Self::IntoSeqIter {
        self.collect_vec_list().into_iter().flatten()
    }
}

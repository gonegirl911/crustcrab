use crate::server::game::world::chunk::Chunk;
use nalgebra::{Point, Scalar};
use rayon::iter::ParallelIterator;
use std::{
    collections::linked_list,
    iter::{self, Flatten},
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
    T: Mul<f32, Output = Self> + Add<Output = Self>,
{
    fn lerp(self, other: Self, t: f32) -> Self {
        self * (1.0 - t) + other * t
    }
}

// ------------------------------------------------------------------------------------------------

pub fn magnitude_squared<const N: usize>(a: Point<i32, N>, b: Point<i32, N>) -> u128 {
    iter::zip(&a.coords, &b.coords)
        .map(|(a, &b)| (a.abs_diff(b) as u128).pow(2))
        .sum()
}

// ------------------------------------------------------------------------------------------------

pub fn chunk_coords<W: WorldCoords>(coords: W) -> W::Point<i32> {
    coords.chunk_coords()
}

pub fn block_coords<W: WorldCoords>(coords: W) -> W::Point<u8> {
    coords.block_coords()
}

pub impl(self) trait WorldCoords {
    type Point<T: Scalar>;

    fn chunk_coords(self) -> Self::Point<i32>;

    fn block_coords(self) -> Self::Point<u8>;
}

impl<const D: usize> WorldCoords for Point<i64, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| c.div_floor(Chunk::DIM as i64) as i32)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as i64) as u8)
    }
}

impl<const D: usize> WorldCoords for Point<f32, D> {
    type Point<T: Scalar> = Point<T, D>;

    fn chunk_coords(self) -> Self::Point<i32> {
        self.map(|c| (c / Chunk::DIM as f32).floor() as i32)
    }

    fn block_coords(self) -> Self::Point<u8> {
        self.map(|c| c.rem_euclid(Chunk::DIM as f32) as u8)
    }
}

pub fn coords<const D: usize>(
    chunk_coords: Point<i32, D>,
    block_coords: Point<u8, D>,
) -> Point<i64, D> {
    chunk_coords.cast() * Chunk::DIM as i64 + block_coords.cast().coords
}

// ------------------------------------------------------------------------------------------------

pub impl(self) trait ParallelIteratorExt: ParallelIterator {
    fn into_seq_iter(self) -> IntoSeqIter<Self::Item> {
        self.collect_vec_list().into_iter().flatten()
    }
}

type IntoSeqIter<T> = Flatten<linked_list::IntoIter<Vec<T>>>;

impl<I: ParallelIterator> ParallelIteratorExt for I {}

pub mod data;

pub use data::Data;

use crate::grid;
use bitvec::BitArr;
use nalgebra::Point3;

pub struct Chunk;

impl Chunk {
    pub const DIM: usize = 16;
    const VOLUME: usize = Self::DIM.pow(3);

    pub fn points() -> impl Iterator<Item = Point3<u8>> {
        Self::layout()
            .into_points()
            .map(|coords| coords.map(|c| c as u8))
    }

    pub fn is_in_bounds(coords: Point3<i8>) -> Option<Point3<u8>> {
        const { assert!(Self::DIM <= i8::MAX as usize) };

        coords
            .iter()
            .all(|c| (0..Self::DIM as i8).contains(c))
            .then(|| coords.map(|c| c as u8))
    }

    fn layout() -> grid::Layout3 {
        grid::Layout3::unit().scale(Self::DIM)
    }
}

pub struct BitSet(BitArr!(for Chunk::VOLUME));

impl BitSet {
    pub fn replace(&mut self, coords: Point3<u8>, value: bool) -> bool {
        self.0.replace(Self::index_unchecked(coords), value)
    }

    fn index_unchecked(coords: Point3<u8>) -> usize {
        Chunk::layout().flatten(coords.cast())
    }
}

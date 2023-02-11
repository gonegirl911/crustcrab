use super::chunk::Chunk;
use bitfield::bitfield;
use nalgebra::Point3;
use rustc_hash::FxHashMap;
use std::ops::{Index, IndexMut};

#[derive(Default)]
pub struct Light(FxHashMap<Point3<i32>, LightMap>);

pub struct LightMap([[[LightValue; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl Index<Point3<u8>> for LightMap {
    type Output = LightValue;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for LightMap {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct LightValue(u16);
    u8;
    skylight, set_skylight: 3, 0;
    red, set_red: 7, 4;
    blue, set_blue: 11, 8;
    green, set_green: 15, 12;
}

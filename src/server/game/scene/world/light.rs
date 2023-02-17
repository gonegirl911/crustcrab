use super::{
    block::{BlockArea, Side},
    chunk::{Chunk, ChunkArea},
};
use crate::server::game::{player::Player, scene::world::block::SIDE_DELTAS};
use bitfield::bitfield;
use enum_map::{enum_map, EnumMap};
use nalgebra::{point, Point3};
use rustc_hash::FxHashMap;
use std::{array, ops::Index};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        ChunkAreaLight::from_fn(|delta| {
            let chunk_coords = coords + Player::chunk_coords(delta.cast()).coords;
            let block_coords = delta.map(|c| (c + Chunk::DIM as i8) as u8 % Chunk::DIM as u8);
            self.0
                .get(&chunk_coords)
                .map(|chunk_light| chunk_light[block_coords])
                .unwrap_or_default()
        })
    }
}

#[derive(Clone, Default)]
struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

pub struct ChunkAreaLight([[[BlockLight; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl ChunkAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| {
                    f(point![x as i8, y as i8, z as i8].map(|c| c + BlockArea::RANGE.start))
                })
            })
        }))
    }

    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        let coords = coords.cast();
        BlockAreaLight::from_fn(|delta| self[coords + delta.coords])
    }
}

impl Index<Point3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c - ChunkArea::RANGE.start);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct BlockLight(u16);
    u8;
    skylight, set_skylight: 3, 0;
    torchlight, set_torchlight: 7, 4, 3;
}

impl Default for BlockLight {
    fn default() -> Self {
        let mut value = BlockLight(0);
        value.set_skylight(15);
        value
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| {
                    f(point![x as i8, y as i8, z as i8].map(|c| c + BlockArea::RANGE.start))
                })
            })
        }))
    }

    pub fn side_lights(&self) -> EnumMap<Side, BlockLight> {
        enum_map! {
            side => self[SIDE_DELTAS[side]]
        }
    }
}

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| c - BlockArea::RANGE.start);
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

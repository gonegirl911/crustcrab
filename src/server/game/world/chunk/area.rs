use super::Chunk;
use crate::{
    server::game::world::block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaLight},
    },
    shared::utils,
};
use nalgebra::{Point3, Vector3, point, vector};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    array, iter, mem,
    ops::{Index, IndexMut, Range},
};

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkArea(ChunkAreaDataStore<Block>);

impl ChunkArea {
    const DIM: usize = Chunk::DIM + BlockArea::PADDING * 2;
    const PADDING: usize = BlockArea::PADDING.div_ceil(Chunk::DIM);
    const AXIS_RANGE: Range<i32> = -(Self::PADDING as i32)..1 + Self::PADDING as i32;
    const REM: usize = BlockArea::PADDING % Chunk::DIM;

    pub fn block_area(&self, coords: Point3<u8>) -> BlockArea {
        BlockArea::from_fn(|delta| self[coords.coords.cast() + delta])
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    pub fn block_deltas(delta: Vector3<i32>) -> impl Iterator<Item = (Point3<u8>, Vector3<i8>)> {
        let [dx, dy, dz] = delta.into();
        Self::block_axis_range(dx).flat_map(move |x| {
            Self::block_axis_range(dy).flat_map(move |y| {
                Self::block_axis_range(dz).map(move |z| {
                    (
                        point![x, y, z],
                        utils::coords((vector![dx, dy, dz], vector![x, y, z])).cast(),
                    )
                })
            })
        })
    }

    fn block_axis_range(dc: i32) -> Range<u8> {
        if dc == Self::AXIS_RANGE.start {
            (Chunk::DIM - Self::REM) as u8..Chunk::DIM as u8
        } else if dc == Self::AXIS_RANGE.end - 1 {
            0..Self::REM as u8
        } else {
            0..Chunk::DIM as u8
        }
    }
}

impl Index<Vector3<i8>> for ChunkArea {
    type Output = Block;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.0[delta]
    }
}

impl IndexMut<Vector3<i8>> for ChunkArea {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        &mut self.0[delta]
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkAreaLight(ChunkAreaDataStore<BlockLight>);

impl ChunkAreaLight {
    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        BlockAreaLight::from_fn(|delta| self[coords.coords.cast() + delta])
    }
}

impl Index<Vector3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.0[delta]
    }
}

impl IndexMut<Vector3<i8>> for ChunkAreaLight {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        &mut self.0[delta]
    }
}

#[derive(Default)]
struct ChunkAreaDataStore<T>([[[T; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl<T> ChunkAreaDataStore<T> {
    fn from_fn<F: FnMut(Vector3<i8>) -> T>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(Self::delta_unchecked([x, y, z]))))
        }))
    }

    fn values(&self) -> impl Iterator<Item = &T> {
        self.0.iter().flatten().flatten()
    }

    fn delta_unchecked(index: [usize; 3]) -> Vector3<i8> {
        index.map(|c| c as i8 - ChunkArea::PADDING as i8).into()
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl<T> Index<Vector3<i8>> for ChunkAreaDataStore<T> {
    type Output = T;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

impl<T> IndexMut<Vector3<i8>> for ChunkAreaDataStore<T> {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &mut self.0[x][y][z]
    }
}

impl<T: PartialEq + Serialize> Serialize for ChunkAreaDataStore<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(pack_iter(self.values()))
    }
}

impl<'de, T: Deserialize<'de> + Clone> Deserialize<'de> for ChunkAreaDataStore<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut iter = unpack_iter(Vec::deserialize(deserializer)?);
        Ok(Self::from_fn(|_| {
            iter.next().unwrap_or_else(|| unreachable!())
        }))
    }
}

fn pack_iter<I>(iter: I) -> impl Iterator<Item = (I::Item, u16)>
where
    I: IntoIterator<Item: PartialEq>,
{
    let mut prev = None;
    let mut count = 1;
    iter.into_iter().filter_map(move |value| {
        if prev.as_ref() == Some(&value) {
            count += 1;
            None
        } else {
            Some((prev.replace(value)?, mem::replace(&mut count, 1)))
        }
    })
}

fn unpack_iter<I, T>(iter: I) -> impl Iterator<Item = T>
where
    I: IntoIterator<Item = (T, u16)>,
    T: Clone,
{
    iter.into_iter()
        .flat_map(|(value, count)| iter::repeat_n(value, count as usize))
}

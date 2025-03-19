use super::Chunk;
use crate::{
    server::game::world::block::{
        Block, BlockLight,
        area::{BlockArea, BlockAreaLight},
    },
    shared::utils,
};
use nalgebra::{Point3, Vector3, point, vector};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{
    fmt::{self, Formatter},
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Index, IndexMut, Range},
    slice,
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
    fn first(&self) -> &T {
        &self.0[0][0][0]
    }

    fn values(&self) -> slice::Iter<T> {
        self.0.as_flattened().as_flattened().iter()
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl<T: PartialEq> ChunkAreaDataStore<T> {
    fn packed_len(&self) -> usize {
        let mut prev = self.first();
        let mut len = 1;

        for value in self.values() {
            if prev != value {
                prev = value;
                len += 1;
            }
        }

        len
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
        const { assert!(ChunkArea::DIM.pow(3) <= u16::MAX as usize) };

        let mut seq = serializer.serialize_seq(Some(self.packed_len()))?;
        let mut prev = self.first();
        let mut count = 0u16;

        for value in self.values() {
            if prev == value {
                count += 1;
            } else {
                seq.serialize_element(&(prev, count))?;
                prev = value;
                count = 1;
            }
        }

        seq.serialize_element(&(prev, count))?;

        seq.end()
    }
}

impl<'de, T: Deserialize<'de> + Clone> Deserialize<'de> for ChunkAreaDataStore<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SeqVisitor<T>(PhantomData<fn() -> ChunkAreaDataStore<T>>);

        impl<'de, T: Deserialize<'de> + Clone> Visitor<'de> for SeqVisitor<T> {
            type Value = ChunkAreaDataStore<T>;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                write!(f, "a sequence of (value, count) pairs")
            }

            fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Self::Value, S::Error> {
                const { assert!(!mem::needs_drop::<T>()) };

                let mut uninit = [const { MaybeUninit::uninit() }; ChunkArea::DIM.pow(3)];
                let mut cur = 0;

                while let Some((value, count)) = seq.next_element::<(T, u16)>()? {
                    let count = count as usize;
                    MaybeUninit::fill(&mut uninit[cur..cur + count], value);
                    cur += count;
                }

                assert!(cur == uninit.len());
                Ok(ChunkAreaDataStore(unsafe { mem::transmute_copy(&uninit) }))
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData))
    }
}

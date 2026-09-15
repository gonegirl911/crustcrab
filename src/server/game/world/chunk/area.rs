use super::Chunk;
use crate::server::game::world::block::{
    Block, BlockLight,
    area::{BlockArea, BlockAreaView, BlockLightAreaView},
};
use nalgebra::{Point3, Vector3, vector};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{
    fmt::{self, Formatter},
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Index, Range},
};

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkArea(ChunkAreaDataStore<Block>);

impl ChunkArea {
    const PADDING: usize = BlockArea::PADDING;
    pub const CHUNK_PADDING: usize = Self::PADDING.div_ceil(Chunk::DIM);
    const DIM: usize = Chunk::DIM + 2 * Self::PADDING;

    pub fn block_area_view(&self, coords: Point3<u8>) -> BlockAreaView<'_> {
        BlockAreaView::new(&self.0, coords)
    }

    pub fn copy_row(&mut self, delta: Vector3<i8>, src: &[Block]) {
        self.0.copy_row(delta, src);
    }

    pub fn chunk_points(coords: Point3<i32>) -> impl Iterator<Item = Point3<i32>> {
        Self::chunk_deltas().map(move |delta| coords + delta)
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        let padding = Self::CHUNK_PADDING as i32;
        (-padding..1 + padding).flat_map(move |dx| {
            (-padding..1 + padding)
                .flat_map(move |dy| (-padding..1 + padding).map(move |dz| vector![dx, dy, dz]))
        })
    }

    pub fn axis_range(dc: i32) -> Range<u8> {
        let dim = Chunk::DIM as i32;
        let padding = Self::PADDING as i32;
        let start = (-padding - dc * dim).max(0);
        let end = (dim + padding - dc * dim).min(dim);
        start as u8..end as u8
    }
}

impl Index<Vector3<i8>> for ChunkArea {
    type Output = Block;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.0[delta]
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct ChunkLightArea(ChunkAreaDataStore<BlockLight>);

impl ChunkLightArea {
    pub fn block_light_area_view(&self, coords: Point3<u8>) -> BlockLightAreaView<'_> {
        BlockLightAreaView::new(&self.0, coords)
    }

    pub fn copy_row(&mut self, delta: Vector3<i8>, src: &[BlockLight]) {
        self.0.copy_row(delta, src);
    }
}

impl Index<Vector3<i8>> for ChunkLightArea {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.0[delta]
    }
}

#[derive(Default)]
pub struct ChunkAreaDataStore<T>([[[T; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl<T> ChunkAreaDataStore<T> {
    fn as_slice(&self) -> &[T] {
        self.0.as_flattened().as_flattened()
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + ChunkArea::PADDING as i8) as usize)
            .into()
    }
}

impl<T: Copy> ChunkAreaDataStore<T> {
    fn copy_row(&mut self, delta: Vector3<i8>, src: &[T]) {
        let [x, y, z] = Self::index_unchecked(delta);
        self.0[x][y][z..][..src.len()].copy_from_slice(src);
    }
}

impl<T: PartialEq> ChunkAreaDataStore<T> {
    fn packed_len(&self) -> usize {
        let mut values = self.as_slice().iter();
        let mut prev = values.next().unwrap();
        let mut len = 1;

        for cur in values {
            if prev != cur {
                prev = cur;
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

impl<T: PartialEq + Serialize> Serialize for ChunkAreaDataStore<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const { assert!(ChunkArea::DIM.pow(3) <= u16::MAX as usize) };

        let mut seq = serializer.serialize_seq(Some(self.packed_len()))?;
        let mut values = self.as_slice().iter();
        let mut prev = values.next().unwrap();
        let mut count = 1u16;

        for cur in values {
            if prev == cur {
                count += 1;
            } else {
                seq.serialize_element(&(prev, count))?;
                prev = cur;
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
                    uninit[cur..cur + count].write_filled(value);
                    cur += count;
                }

                if cur == uninit.len() {
                    Ok(ChunkAreaDataStore(unsafe { mem::transmute_copy(&uninit) }))
                } else {
                    Err(de::Error::invalid_length(
                        cur,
                        &&*format!("unpacked length of {}", uninit.len()),
                    ))
                }
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData))
    }
}

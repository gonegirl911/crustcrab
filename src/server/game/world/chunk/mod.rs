pub mod generator;
pub mod light;

use self::light::ChunkAreaLight;
use super::{
    action::BlockAction,
    block::{data::BlockData, Block, BlockArea},
    World,
};
use crate::{
    client::game::world::BlockVertex,
    shared::{
        bound::{Aabb, BoundingSphere},
        utils,
    },
};
use bitvec::BitArr;
use nalgebra::{point, vector, Point3, Vector3};
use std::{
    array, mem,
    ops::{Index, IndexMut, Range},
};

#[repr(align(16))]
#[derive(Clone, Default)]
pub struct Chunk([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl Chunk {
    pub const DIM: usize = 16;

    fn from_fn<F: FnMut(Point3<u8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(point![x, y, z].cast())))
        }))
    }

    pub fn vertices<'a>(
        &'a self,
        area: &'a ChunkArea,
        area_light: &'a ChunkAreaLight,
    ) -> impl Iterator<Item = (&'static BlockData, impl Iterator<Item = BlockVertex>)> + 'a {
        self.blocks().map(|(coords, block)| {
            let data = block.data();
            (
                data,
                data.vertices(
                    coords,
                    area.block_area(coords),
                    area_light.block_area_light(coords),
                ),
            )
        })
    }

    fn blocks(&self) -> impl Iterator<Item = (Point3<u8>, Block)> + '_ {
        Self::points().zip(self.0.iter().flatten().flatten().copied())
    }

    pub fn is_empty(&self) -> bool {
        let expected = unsafe { mem::transmute([Block::Air; Self::DIM]) };
        self.0
            .iter()
            .flatten()
            .all(|blocks| *unsafe { mem::transmute::<_, &u128>(blocks) } == expected)
    }

    pub fn apply(&mut self, coords: Point3<u8>, action: &BlockAction) -> bool {
        self[coords].apply(action)
    }

    pub fn apply_unchecked(&mut self, coords: Point3<u8>, action: &BlockAction) {
        self[coords].apply_unchecked(action)
    }

    fn points() -> impl Iterator<Item = Point3<u8>> {
        (0..Self::DIM).flat_map(|x| {
            (0..Self::DIM).flat_map(move |y| (0..Self::DIM).map(move |z| point![x, y, z].cast()))
        })
    }

    fn bounding_box(coords: Point3<i32>) -> Aabb {
        Aabb::new(
            World::coords(coords, Default::default()).cast(),
            Vector3::from_element(Self::DIM).cast(),
        )
    }

    pub fn bounding_sphere(coords: Point3<i32>) -> BoundingSphere {
        Self::bounding_box(coords).into()
    }
}

impl Index<Point3<u8>> for Chunk {
    type Output = Block;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for Chunk {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

#[derive(Default)]
pub struct ChunkArea(BitArr!(for Self::DIM * Self::DIM * Self::DIM, in usize));

impl ChunkArea {
    pub const DIM: usize = Chunk::DIM + Self::PADDING * 2;
    pub const PADDING: usize = BlockArea::PADDING;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..(Chunk::DIM + Self::PADDING) as i8;
    const CHUNK_PADDING: usize = utils::div_ceil(Self::PADDING, Chunk::DIM);
    const CHUNK_AXIS_RANGE: Range<i32> =
        -(Self::CHUNK_PADDING as i32)..1 + Self::CHUNK_PADDING as i32;
    const REM: usize = Self::PADDING % Chunk::DIM;

    fn block_area(&self, coords: Point3<u8>) -> BlockArea {
        let coords = coords.coords.cast();
        BlockArea::from_fn(|delta| self.is_opaque(coords + delta))
    }

    fn is_opaque(&self, delta: Vector3<i8>) -> bool {
        unsafe { *self.0.get_unchecked(Self::index(delta)) }
    }

    pub fn set(&mut self, delta: Vector3<i8>, is_opaque: bool) {
        unsafe {
            self.0.set_unchecked(Self::index(delta), is_opaque);
        }
    }

    pub fn deltas() -> impl Iterator<
        Item = (
            Vector3<i32>,
            impl Iterator<Item = (Point3<u8>, Vector3<i8>)>,
        ),
    > {
        Self::CHUNK_AXIS_RANGE.flat_map(|dx| {
            Self::CHUNK_AXIS_RANGE.flat_map(move |dy| {
                Self::CHUNK_AXIS_RANGE.map(move |dz| {
                    (
                        vector![dx, dy, dz],
                        Self::block_axis_range(dx).flat_map(move |x| {
                            Self::block_axis_range(dy).flat_map(move |y| {
                                Self::block_axis_range(dz).map(move |z| {
                                    (
                                        point![x, y, z],
                                        World::coords(point![dx, dy, dz], point![x, y, z])
                                            .coords
                                            .cast(),
                                    )
                                })
                            })
                        }),
                    )
                })
            })
        })
    }

    pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i32>> {
        Self::CHUNK_AXIS_RANGE.flat_map(|dx| {
            Self::CHUNK_AXIS_RANGE
                .flat_map(move |dy| Self::CHUNK_AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    fn index(delta: Vector3<i8>) -> usize {
        assert!(
            Self::AXIS_RANGE.contains(&delta.x)
                && Self::AXIS_RANGE.contains(&delta.y)
                && Self::AXIS_RANGE.contains(&delta.z)
        );
        unsafe { Self::index_unchecked(delta) }
    }

    unsafe fn index_unchecked(delta: Vector3<i8>) -> usize {
        let idx = delta.map(|c| (c + Self::PADDING as i8) as usize);
        idx.x * Self::DIM.pow(2) + idx.y * Self::DIM + idx.z
    }

    fn block_axis_range(dc: i32) -> Range<u8> {
        if dc == Self::CHUNK_AXIS_RANGE.start {
            (Chunk::DIM - Self::REM) as u8..Chunk::DIM as u8
        } else if dc == Self::CHUNK_AXIS_RANGE.end - 1 {
            0..Self::REM as u8
        } else {
            0..Chunk::DIM as u8
        }
    }
}

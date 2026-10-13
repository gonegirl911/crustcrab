use crate::{
    server::game::{
        block::{Block, BlockLight},
        chunk::{Chunk, ChunkLight},
    },
    shared::cuboid::Cuboid,
};
use nalgebra::{Point3, Vector3};
use std::{array, cell::Cell, mem::DropGuard, ops::Index, sync::Arc};

pub fn compute(
    RegionInput {
        coords,
        chunks: input_chunks,
        lights: input_lights,
        has_priority,
    }: RegionInput,
) -> RegionOutput {
    let blocks = DropGuard::new(BLOCK_FLAT_GRID_SCRATCH.take().unwrap(), |scratch| {
        BLOCK_FLAT_GRID_SCRATCH.set(Some(scratch));
    });
    let lights = DropGuard::new(BLOCK_LIGHT_FLAT_GRID_SCRATCH.take().unwrap(), |scratch| {
        BLOCK_LIGHT_FLAT_GRID_SCRATCH.set(Some(scratch));
    });

    RegionOutput {
        coords,
        input_chunks,
        input_lights,
        has_priority,
    }
}

pub struct RegionInput {
    pub coords: Point3<i32>,
    pub chunks: SlotGrid<Chunk>,
    pub lights: SlotGrid<ChunkLight>,
    pub has_priority: bool,
}

pub struct RegionOutput {
    pub coords: Point3<i32>,
    pub input_chunks: SlotGrid<Chunk>,
    pub input_lights: SlotGrid<ChunkLight>,
    pub has_priority: bool,
}

pub struct SlotGrid<T>(pub [Option<Arc<T>>; CHUNK_VOLUME]);

impl<T> SlotGrid<T> {
    pub fn from_fn<F: FnMut(Vector3<i32>) -> Option<Arc<T>>>(mut f: F) -> Self {
        Self(array::from_fn(|i| f(Self::delta_unchecked(i))))
    }

    pub fn kernel(&self) -> &Option<Arc<T>> {
        &self[Default::default()]
    }

    fn flatten_into<U>(&self, grid: &mut FlatGrid<U>)
    where
        T: AsRef<[U]>,
        U: Clone + Default,
    {
        for (i, slot) in self.0.iter().enumerate() {
            let section = &mut grid.0[i * Chunk::VOLUME..][..Chunk::VOLUME];
            if let Some(value) = slot {
                section.clone_from_slice((**value).as_ref());
            } else {
                section.fill(Default::default());
            }
        }
    }

    fn delta_unchecked(index: usize) -> Vector3<i32> {
        chunk_cuboid().unflatten(index as i64).coords.cast()
    }

    fn index_unchecked(delta: Vector3<i8>) -> usize {
        chunk_cuboid().flatten(delta.cast().into()) as usize
    }
}

impl<T> Index<Vector3<i8>> for SlotGrid<T> {
    type Output = Option<Arc<T>>;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.0[Self::index_unchecked(delta)]
    }
}

struct FlatGrid<T>(Box<[T]>);

impl<T: Default> Default for FlatGrid<T> {
    fn default() -> Self {
        Self(Vec::from_fn(VOLUME, |_| Default::default()).into())
    }
}

thread_local! {
    static BLOCK_FLAT_GRID_SCRATCH: Cell<Option<FlatGrid<Block>>> =
        Cell::new(Some(Default::default()));
    static BLOCK_LIGHT_FLAT_GRID_SCRATCH: Cell<Option<FlatGrid<BlockLight>>> =
        Cell::new(Some(Default::default()));
}

pub fn chunk_deltas() -> impl Iterator<Item = Vector3<i8>> {
    chunk_cuboid()
        .into_points()
        .map(|coords| coords.coords.cast())
}

fn chunk_cuboid() -> Cuboid {
    Cuboid::unit().pad(CHUNK_PADDING as i64)
}

pub const LIGHT_REACH: usize = BlockLight::COMPONENT_MAX as usize - 1;
const PADDING: usize = LIGHT_REACH;
const CHUNK_PADDING: usize = PADDING.div_ceil(Chunk::DIM);
const CHUNK_DIM: usize = 1 + 2 * CHUNK_PADDING;
const DIM: usize = CHUNK_DIM * Chunk::DIM;
const CHUNK_VOLUME: usize = CHUNK_DIM.pow(3);
const VOLUME: usize = DIM.pow(3);

use super::{
    Block, BlockLight,
    data::{Component, Corner, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS, Side},
};
use crate::{
    enum_map, server::game::world::chunk::area::ChunkAreaDataStore, shared::enum_map::EnumMap,
};
use nalgebra::{Point3, Vector3};
use std::{array, ops::Index};

pub struct BlockContext<S> {
    source: S,
}

impl<S: BlockAreaSource> BlockContext<S> {
    pub fn is_side_visible(&self, side: Option<Side>) -> bool {
        side.is_none_or(|side| {
            let neighbor = self.block(SIDE_DELTAS[side]);
            neighbor != self.kernel() && !neighbor.data().is_opaque()
        })
    }

    pub fn corner_aos(&self, side: Option<Side>, is_externally_lit: bool) -> EnumMap<Corner, u8> {
        if is_externally_lit && let Some(side) = side {
            enum_map! { corner => self.ao(side, corner) }
        } else {
            Default::default()
        }
    }

    fn ao(&self, side: Side, corner: Corner) -> u8 {
        let components = SIDE_CORNER_COMPONENT_DELTAS[side][corner]
            .map(|_, delta| self.block(delta).data().is_opaque());
        let edge1 = components[Component::Edge1];
        let edge2 = components[Component::Edge2];
        let corner = components[Component::Corner];
        if edge1 && edge2 {
            3
        } else {
            edge1 as u8 + edge2 as u8 + corner as u8
        }
    }
}

impl<S: BlockLightAreaSource> BlockContext<S> {
    pub fn corner_lights(
        &self,
        side: Option<Side>,
        area: &BlockContext<impl BlockAreaSource>,
    ) -> EnumMap<Corner, BlockLight> {
        let light = self.kernel();
        if let Some(side) = side {
            SIDE_CORNER_COMPONENT_DELTAS[side].map(move |_, component_deltas| {
                self.smooth_lighting(side, area, component_deltas)
                    .sup(light)
            })
        } else {
            enum_map! { _ => light }
        }
    }

    fn smooth_lighting(
        &self,
        side: Side,
        area: &BlockContext<impl BlockAreaSource>,
        component_deltas: EnumMap<Component, Vector3<i8>>,
    ) -> BlockLight {
        let (count, sum) = component_deltas
            .into_values()
            .chain([SIDE_DELTAS[side]])
            .filter(|&delta| !area.block(delta).data().is_opaque())
            .map(|delta| self.block_light(delta))
            .fold((0, [0; _]), |(count, sum), light| {
                (count + 1, array::from_fn(|i| sum[i] + light.component(i)))
            });

        sum.map(|c| c / count.max(1)).into()
    }
}

impl<T> BlockContext<BlockAreaDataStore<T>> {
    pub fn from_fn<F: FnMut(Vector3<i8>) -> T>(f: F) -> Self {
        Self {
            source: BlockAreaDataStore::from_fn(f),
        }
    }
}

impl<'a, T> BlockContext<BlockAreaDataRef<'a, T>> {
    pub fn new(data: &'a ChunkAreaDataStore<T>, coords: Point3<u8>) -> Self {
        Self {
            source: BlockAreaDataRef { data, coords },
        }
    }
}

impl<S: BlockAreaSource> BlockAreaSource for BlockContext<S> {
    fn block(&self, delta: Vector3<i8>) -> Block {
        self.source.block(delta)
    }
}

impl<S: BlockLightAreaSource> BlockLightAreaSource for BlockContext<S> {
    fn block_light(&self, delta: Vector3<i8>) -> BlockLight {
        self.source.block_light(delta)
    }
}

pub type BlockArea = BlockContext<BlockAreaDataStore<Block>>;

impl BlockArea {
    pub const PADDING: usize = 1;
    const DIM: usize = 1 + Self::PADDING * 2;
}

pub type BlockAreaView<'a> = BlockContext<BlockAreaDataRef<'a, Block>>;

pub type BlockLightArea = BlockContext<BlockAreaDataStore<BlockLight>>;

pub type BlockLightAreaView<'a> = BlockContext<BlockAreaDataRef<'a, BlockLight>>;

pub trait BlockAreaSource {
    fn block(&self, delta: Vector3<i8>) -> Block;

    fn kernel(&self) -> Block {
        self.block(Default::default())
    }
}

pub trait BlockLightAreaSource {
    fn block_light(&self, delta: Vector3<i8>) -> BlockLight;

    fn kernel(&self) -> BlockLight {
        self.block_light(Default::default())
    }
}

pub struct BlockAreaDataStore<T>([[[T; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl<T> BlockAreaDataStore<T> {
    fn from_fn<F: FnMut(Vector3<i8>) -> T>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(Self::delta_unchecked([x, y, z]))))
        }))
    }

    fn delta_unchecked(index: [usize; 3]) -> Vector3<i8> {
        index.map(|c| c as i8 - BlockArea::PADDING as i8).into()
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl<T> Index<Vector3<i8>> for BlockAreaDataStore<T> {
    type Output = T;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

impl BlockAreaSource for BlockAreaDataStore<Block> {
    fn block(&self, delta: Vector3<i8>) -> Block {
        self[delta]
    }
}

impl BlockLightAreaSource for BlockAreaDataStore<BlockLight> {
    fn block_light(&self, delta: Vector3<i8>) -> BlockLight {
        self[delta]
    }
}

pub struct BlockAreaDataRef<'a, T> {
    data: &'a ChunkAreaDataStore<T>,
    coords: Point3<u8>,
}

impl<T> Index<Vector3<i8>> for BlockAreaDataRef<'_, T> {
    type Output = T;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        &self.data[self.coords.coords.cast() + delta]
    }
}

impl BlockAreaSource for BlockAreaDataRef<'_, Block> {
    fn block(&self, delta: Vector3<i8>) -> Block {
        self[delta]
    }
}

impl BlockLightAreaSource for BlockAreaDataRef<'_, BlockLight> {
    fn block_light(&self, delta: Vector3<i8>) -> BlockLight {
        self[delta]
    }
}

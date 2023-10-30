use super::{
    data::{Component, Corner, Side, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS},
    Block, BlockLight,
};
use enum_map::{enum_map, EnumMap};
use nalgebra::{point, vector, Point3, Vector3};
use std::{
    array,
    ops::{Index, IndexMut, Range},
};

#[derive(Clone, Copy, Default)]
pub struct BlockArea([[[Block; Self::DIM]; Self::DIM]; Self::DIM]);

impl BlockArea {
    const DIM: usize = 1 + Self::PADDING * 2;
    pub const PADDING: usize = 1;
    const AXIS_RANGE: Range<i8> = -(Self::PADDING as i8)..1 + Self::PADDING as i8;

    pub fn from_fn<F: FnMut(Vector3<i8>) -> Block>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(unsafe { Self::delta_unchecked(point![x, y, z]) }))
            })
        }))
    }

    pub fn is_side_visible(self, side: Side) -> bool {
        let neighbor = self[SIDE_DELTAS[side]];
        neighbor != self.block() && neighbor.data().is_transparent()
    }

    pub fn corner_aos(self, side: Side, is_externally_lit: bool) -> EnumMap<Corner, u8> {
        if is_externally_lit {
            enum_map! { corner => self.ao(side, corner) }
        } else {
            enum_map! { _ => self.internal_ao() }
        }
    }

    pub fn internal_ao(self) -> u8 {
        3
    }

    pub fn block(self) -> Block {
        self[Default::default()]
    }

    fn block_mut(&mut self) -> &mut Block {
        &mut self[Default::default()]
    }

    fn ao(self, side: Side, corner: Corner) -> u8 {
        let components = self.components(side, corner);

        let [edge1, edge2, corner] = [
            components[Component::Edge1],
            components[Component::Edge2],
            components[Component::Corner],
        ];

        if edge1 && edge2 {
            0
        } else {
            3 - (edge1 as u8 + edge2 as u8 + corner as u8)
        }
    }

    fn components(self, side: Side, corner: Corner) -> EnumMap<Component, bool> {
        SIDE_CORNER_COMPONENT_DELTAS[side][corner].map(|_, delta| self[delta].data().is_opaque())
    }

    pub fn deltas() -> impl Iterator<Item = Vector3<i8>> {
        Self::AXIS_RANGE.flat_map(|dx| {
            Self::AXIS_RANGE.flat_map(move |dy| Self::AXIS_RANGE.map(move |dz| vector![dx, dy, dz]))
        })
    }

    unsafe fn delta_unchecked(index: Point3<usize>) -> Vector3<i8> {
        index.coords.map(|c| c as i8 - BlockArea::PADDING as i8)
    }

    unsafe fn index_unchecked(delta: Vector3<i8>) -> Point3<usize> {
        delta
            .map(|c| (c + BlockArea::PADDING as i8) as usize)
            .into()
    }
}

impl From<Block> for BlockArea {
    fn from(block: Block) -> Self {
        let mut value = Self::default();
        *value.block_mut() = block;
        value
    }
}

impl Index<Vector3<i8>> for BlockArea {
    type Output = Block;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &self.0[idx.x][idx.y][idx.z]
    }
}

impl IndexMut<Vector3<i8>> for BlockArea {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let idx = unsafe { Self::index_unchecked(delta) };
        &mut self.0[idx.x][idx.y][idx.z]
    }
}

#[derive(Default)]
pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    pub fn from_fn<F: FnMut(Vector3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(unsafe { BlockArea::delta_unchecked(point![x, y, z]) }))
            })
        }))
    }

    pub fn corner_lights(
        &self,
        side: Side,
        area: BlockArea,
        is_externally_lit: bool,
    ) -> EnumMap<Corner, BlockLight> {
        if is_externally_lit {
            SIDE_CORNER_COMPONENT_DELTAS[side].map(|_, component_deltas| {
                let (count, sum) = component_deltas
                    .into_values()
                    .chain([SIDE_DELTAS[side]])
                    .filter(|&delta| area[delta].data().is_transparent())
                    .map(|delta| self[delta])
                    .fold((0, [0; BlockLight::LEN]), |(count, sum), light| {
                        (count + 1, array::from_fn(|i| sum[i] + light.component(i)))
                    });
                sum.map(|c| c / count.max(1)).into()
            })
        } else {
            enum_map! { _ => self.internal_light() }
        }
    }

    pub fn internal_light(&self) -> BlockLight {
        self.block_light()
    }

    fn block_light(&self) -> BlockLight {
        self[Default::default()]
    }
}

impl Index<Vector3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = unsafe { BlockArea::index_unchecked(delta) };
        &self.0[idx.x][idx.y][idx.z]
    }
}

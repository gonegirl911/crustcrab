use super::{
    Block, BlockLight,
    data::{Component, Corner, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS, Side},
};
use crate::{enum_map, shared::enum_map::EnumMap};
use nalgebra::{Vector3, vector};
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
            array::from_fn(|y| array::from_fn(|z| f(Self::delta_unchecked([x, y, z]))))
        }))
    }

    pub fn is_side_visible(self, side: Option<Side>) -> bool {
        side.map_or(true, |side| {
            let neighbor = self[SIDE_DELTAS[side]];
            neighbor != self.block() && neighbor.data().is_transparent()
        })
    }

    pub fn corner_aos(self, side: Option<Side>, is_externally_lit: bool) -> EnumMap<Corner, u8> {
        if is_externally_lit && let Some(side) = side {
            enum_map! { corner => self.ao(side, corner) }
        } else {
            enum_map! { _ => 3 }
        }
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

    fn delta_unchecked(index: [usize; 3]) -> Vector3<i8> {
        index.map(|c| c as i8 - Self::PADDING as i8).into()
    }

    fn index_unchecked(delta: Vector3<i8>) -> [usize; 3] {
        delta.map(|c| (c + Self::PADDING as i8) as usize).into()
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
        let [x, y, z] = Self::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

impl IndexMut<Vector3<i8>> for BlockArea {
    fn index_mut(&mut self, delta: Vector3<i8>) -> &mut Self::Output {
        let [x, y, z] = Self::index_unchecked(delta);
        &mut self.0[x][y][z]
    }
}

#[derive(Default)]
pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    pub fn from_fn<F: FnMut(Vector3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| array::from_fn(|z| f(BlockArea::delta_unchecked([x, y, z]))))
        }))
    }

    pub fn corner_lights(
        &self,
        side: Option<Side>,
        area: BlockArea,
    ) -> EnumMap<Corner, BlockLight> {
        let light = self.block_light();
        if let Some(side) = side {
            SIDE_CORNER_COMPONENT_DELTAS[side].map(|_, component_deltas| {
                self.smooth_lighting(side, area, component_deltas)
                    .sup(light)
            })
        } else {
            enum_map! { _ => light }
        }
    }

    fn block_light(&self) -> BlockLight {
        self[Default::default()]
    }

    fn smooth_lighting(
        &self,
        side: Side,
        area: BlockArea,
        component_deltas: EnumMap<Component, Vector3<i8>>,
    ) -> BlockLight {
        let (count, sum) = component_deltas
            .into_values()
            .chain([SIDE_DELTAS[side]])
            .filter(|&delta| area[delta].data().is_transparent())
            .map(|delta| self[delta])
            .fold((0, [0; _]), |(count, sum), light| {
                (count + 1, array::from_fn(|i| sum[i] + light.component(i)))
            });

        sum.map(|c| c / count.max(1)).into()
    }
}

impl Index<Vector3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let [x, y, z] = BlockArea::index_unchecked(delta);
        &self.0[x][y][z]
    }
}

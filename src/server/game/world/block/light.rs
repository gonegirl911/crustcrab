use super::{BlockArea, Corner, Side, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS};
use bitfield::bitfield;
use enum_map::EnumMap;
use nalgebra::{vector, Vector3};
use std::{
    array,
    iter::Sum,
    ops::{Add, Index, Range},
};

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct BlockLight(u32);
    pub u8, component, set_component: 3, 0, 6;
}

impl BlockLight {
    pub const SKYLIGHT_RANGE: Range<usize> = 0..3;
    pub const TORCHLIGHT_RANGE: Range<usize> = 3..6;

    pub fn replace_component(&mut self, index: usize, value: u8) -> u8 {
        let component = self.component(index);
        self.set_component(index, value);
        component
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    pub fn from_fn<F: FnMut(Vector3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(vector![x, y, z].map(|c| c as i8 - BlockArea::PADDING as i8)))
            })
        }))
    }

    pub fn corner_lights(&self, side: Side, area: BlockArea) -> EnumMap<Corner, BlockLight> {
        let side_delta = SIDE_DELTAS[side];
        SIDE_CORNER_COMPONENT_DELTAS[side].map(|_, component_deltas| {
            component_deltas
                .into_values()
                .chain([side_delta])
                .filter(|delta| area.is_transparent(*delta))
                .map(|delta| self[delta])
                .sum::<BlockLightSum>()
                .avg()
        })
    }
}

impl Index<Vector3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, delta: Vector3<i8>) -> &Self::Output {
        let idx = delta.map(|c| (c + BlockArea::PADDING as i8) as usize);
        &self.0[idx.x][idx.y][idx.z]
    }
}

#[derive(Default)]
struct BlockLightSum {
    components: [u8; 6],
    count: u8,
}

impl BlockLightSum {
    fn avg(self) -> BlockLight {
        let mut value = BlockLight::default();
        for (i, component) in self.components.into_iter().enumerate() {
            value.set_component(i, component / self.count.max(1))
        }
        value
    }
}

impl Sum<BlockLight> for BlockLightSum {
    fn sum<I: Iterator<Item = BlockLight>>(iter: I) -> Self {
        iter.fold(Default::default(), |accum, light| accum + light)
    }
}

impl Add<BlockLight> for BlockLightSum {
    type Output = Self;

    fn add(self, rhs: BlockLight) -> Self::Output {
        Self {
            components: array::from_fn(|i| self.components[i] + rhs.component(i)),
            count: self.count + 1,
        }
    }
}

use super::{
    block::{
        BlockLight,
        area::{BlockArea, BlockAreaSource, BlockLightArea, BlockLightAreaSource},
    },
    chunk::store::ChunkStore,
    coords,
    light::WorldLight,
};
use crate::{client::game::shading::DAY_LIGHT_TABLE, shared::bound::Aabb};
use nalgebra::Point3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BlockHoverData {
    coords: Point3<i64>,
    area: BlockArea,
    light_area: BlockLightArea,
}

impl BlockHoverData {
    pub fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i64>) -> Self {
        Self {
            coords,
            area: chunks.block_area(coords),
            light_area: light.block_light_area(coords),
        }
    }

    pub fn hitbox(&self) -> Aabb {
        self.area.kernel().data().hitbox(self.coords)
    }

    pub fn brightness(&self) -> BlockLight {
        self.area
            .kernel()
            .data()
            .mesh(coords::block(self.coords), &self.area, &self.light_area)
            .max_by(|a, b| {
                let a = DAY_LIGHT_TABLE.value(a.light(), 0).lum();
                let b = DAY_LIGHT_TABLE.value(b.light(), 0).lum();
                a.total_cmp(&b)
            })
            .map_or(self.light_area.kernel(), |v| v.light())
    }
}

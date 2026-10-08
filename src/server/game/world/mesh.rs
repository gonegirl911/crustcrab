use super::{ChunkStore, WorldLight};
use crate::{
    client::game::world::mesh::BlockVertex,
    server::game::{
        block::{
            Block, BlockLight,
            area::{BlockAreaSource, BlockContext, BlockLightAreaSource},
            data::{Corner, RenderLayer, SIDE_AXES, Side},
        },
        chunk::{
            Chunk,
            area::{ChunkArea, ChunkLightArea},
            visibility::VisibilityGraph,
        },
    },
    shared::enum_map::{Enum, EnumMap},
};
use nalgebra::{Point2, Point3, point};
use serde::{Deserialize, Serialize};
use std::array;

#[derive(Serialize, Deserialize)]
pub struct ChunkData {
    pub coords: Point3<i32>,
    area: ChunkArea,
    light_area: ChunkLightArea,
    pub visibility_graph: VisibilityGraph,
}

impl ChunkData {
    pub fn new(chunks: &ChunkStore, light: &WorldLight, coords: Point3<i32>) -> Self {
        Self {
            coords,
            area: chunks.chunk_area(coords),
            light_area: light.chunk_light_area(coords),
            visibility_graph: chunks[coords].visibility_graph,
        }
    }

    pub fn vertices(&self) -> EnumMap<RenderLayer, Vec<BlockVertex>> {
        let mut vertices = EnumMap::<_, Vec<_>>::default();

        for coords in Chunk::points() {
            let area = self.area.block_area_view(coords);
            let light_area = self.light_area.block_light_area_view(coords);
            let data = area.kernel().data();
            vertices[data.render_layer].extend(data.vertices(
                None,
                coords,
                point![1, 1, 1],
                point![1, 1],
                area.corner_aos(None, data.is_externally_lit()),
                light_area.corner_lights(None, &area),
            ));
        }

        for side in Enum::variants() {
            let axes = SIDE_AXES[side];

            for normal in 0..Chunk::DIM as u8 {
                let mut quads = array::from_fn(|v| {
                    array::from_fn(|u| {
                        let coords = axes.swizzle(point![normal, u as u8, v as u8]);
                        Quad::new(
                            side,
                            &self.area.block_area_view(coords),
                            &self.light_area.block_light_area_view(coords),
                        )
                    })
                });
                let plane = normal + side.is_positive() as u8;

                for v in 0..Chunk::DIM {
                    let mut u = 0;

                    while u < Chunk::DIM {
                        let Some(quad) = quads[v][u] else {
                            u += 1;
                            continue;
                        };
                        let width = Self::merge_width(&quads, v, u, &quad);
                        let height = Self::merge_height(&quads, v, u, &quad, width);

                        vertices[quad.block.data().render_layer].extend(quad.vertices(
                            side,
                            point![plane, u as u8, v as u8],
                            point![width as u8, height as u8],
                        ));

                        for dv in 0..height {
                            quads[v + dv][u..u + width].fill(None);
                        }

                        u += width;
                    }
                }
            }
        }

        vertices
    }

    fn merge_width(
        quads: &[[Option<Quad>; Chunk::DIM]; Chunk::DIM],
        v: usize,
        u: usize,
        quad: &Quad,
    ) -> usize {
        let mut width = 1;
        while u + width < Chunk::DIM && quads[v][u + width].as_ref() == Some(quad) {
            width += 1;
        }
        width
    }

    fn merge_height(
        quads: &[[Option<Quad>; Chunk::DIM]; Chunk::DIM],
        v: usize,
        u: usize,
        quad: &Quad,
        width: usize,
    ) -> usize {
        let mut height = 1;
        'outer: while v + height < Chunk::DIM {
            for du in 0..width {
                if quads[v + height][u + du].as_ref() != Some(quad) {
                    break 'outer;
                }
            }
            height += 1;
        }
        height
    }
}

#[derive(Clone, Copy)]
struct Quad {
    block: Block,
    corner_aos: EnumMap<Corner, u8>,
    corner_lights: EnumMap<Corner, BlockLight>,
}

impl Quad {
    fn new(
        side: Side,
        area: &BlockContext<impl BlockAreaSource>,
        light_area: &BlockContext<impl BlockLightAreaSource>,
    ) -> Option<Self> {
        let block = area.kernel();
        let data = block.data();
        let is_externally_lit = data.is_externally_lit();
        area.is_side_visible(Some(side)).then(|| Self {
            block,
            corner_aos: area.corner_aos(Some(side), is_externally_lit),
            corner_lights: light_area.corner_lights(Some(side), area),
        })
    }

    fn vertices(
        self,
        side: Side,
        coords: Point3<u8>,
        dims: Point2<u8>,
    ) -> impl Iterator<Item = BlockVertex> {
        let axes = SIDE_AXES[side];
        self.block.data().vertices(
            Some(side),
            axes.swizzle(coords),
            axes.swizzle(point![0, dims.x, dims.y]),
            dims,
            self.corner_aos,
            self.corner_lights,
        )
    }
}

impl PartialEq for Quad {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
            && self.corner_aos == other.corner_aos
            && self.corner_lights == other.corner_lights
    }
}

use super::{
    block::{Block, BlockArea, BlockData, Corner, Side, SIDE_CORNER_COMPONENT_DELTAS, SIDE_DELTAS},
    chunk::{BlockAction, Chunk, ChunkArea, ChunkCell, ChunkMap},
};
use bitfield::bitfield;
use enum_map::EnumMap;
use nalgebra::{point, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    array,
    cmp::Ordering,
    collections::VecDeque,
    iter::Sum,
    ops::{Index, IndexMut, Range},
};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        ChunkAreaLight::from_fn(|delta| {
            let delta = delta.cast();
            let chunk_coords = coords + ChunkMap::chunk_coords(delta).coords;
            let block_coords = ChunkMap::block_coords(delta);
            self.0
                .get(&chunk_coords)
                .map(|cell| cell[block_coords])
                .unwrap_or_default()
        })
    }

    pub fn insert(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i32>,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    pub fn remove(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i32>,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    pub fn apply(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i64>> {
        match action {
            BlockAction::Destroy => self.destroy(cells, coords),
            BlockAction::Place(block) => self.place(cells, coords, *block),
        }
    }

    fn destroy(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
    ) -> FxHashSet<Point3<i64>> {
        self.unblock_skylight(cells, coords)
            .into_iter()
            .chain(self.destroy_torchlight(cells, coords))
            .collect()
    }

    fn place(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        self.block_skylight(cells, coords, block)
            .into_iter()
            .chain(self.place_torchlight(cells, coords, block))
            .collect()
    }

    fn block_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    fn place_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        block
            .data()
            .luminance
            .into_iter()
            .zip(BlockLight::TORCHLIGHT_RANGE)
            .flat_map(|(v, i)| self.set_torchlight(cells, coords, i, v))
            .collect()
    }

    fn unblock_skylight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
    ) -> FxHashSet<Point3<i64>> {
        Default::default()
    }

    fn destroy_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
    ) -> FxHashSet<Point3<i64>> {
        BlockLight::TORCHLIGHT_RANGE
            .flat_map(|i| self.unset_torchlight(cells, coords, i))
            .collect()
    }

    fn set_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let component = self.replace_component(coords, index, value);
        match component.cmp(&value) {
            Ordering::Less => self.spread_component(cells, coords, index, value),
            Ordering::Equal => Default::default(),
            Ordering::Greater => self.unspread_component(cells, coords, index, component),
        }
    }

    fn unset_torchlight(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        let component = self.take_component(coords, index);
        if component != 0 {
            self.unspread_component(cells, coords, index, component)
        } else {
            self.spread_neighbors(cells, coords, index)
        }
    }

    fn spread_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut visits = FxHashSet::from_iter([coords]);

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                if let Some(value) = self.set_component(cells, coords, index, value - 1) {
                    deq.push_back((coords, value));
                }
            }
        }

        visits
    }

    fn unspread_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut sources = vec![];
        let mut visits = FxHashSet::from_iter([coords]);

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                match self.unset_component(cells, coords, index, value - 1) {
                    Ok(value) => deq.push_back((coords, value)),
                    Err(0) => {}
                    Err(component) => sources.push((coords, component)),
                }
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, component)| self.spread_component(cells, coords, index, component))
            .chain(visits)
            .collect()
    }

    fn spread_neighbors(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        Self::neighbors(coords)
            .filter_map(|coords| {
                let component = self.component(coords, index);
                (component != 0).then(|| self.spread_component(cells, coords, index, component))
            })
            .flatten()
            .collect()
    }

    fn replace_component(&mut self, coords: Point3<i64>, index: usize, value: u8) -> u8 {
        self.block_light_mut(&LightNode::new(coords))
            .replace_component(index, value)
    }

    fn take_component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
    }

    fn component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light_mut(&LightNode::new(coords))
            .component(index)
    }

    fn set_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Option<u8> {
        let node = LightNode::new(coords);
        let block_light = self.block_light_mut(&node);
        let component = block_light.component(index);
        let value = node.apply_filter(cells, index, value);
        (component < value).then(|| {
            block_light.set_component(index, value);
            value
        })
    }

    fn unset_component(
        &mut self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Result<u8, u8> {
        let node = LightNode::new(coords);
        let block_light = self.block_light_mut(&node);
        let component = block_light.component(index);
        if component != 0 && component == node.apply_filter(cells, index, value) {
            block_light.set_component(index, 0);
            Ok(value)
        } else {
            Err(component)
        }
    }

    fn block_light_mut(&mut self, node: &LightNode) -> &mut BlockLight {
        &mut self.0.entry(node.chunk_coords).or_default()[node.block_coords]
    }

    fn unvisited_neighbors(
        coords: Point3<i64>,
        visits: &mut FxHashSet<Point3<i64>>,
    ) -> impl Iterator<Item = Point3<i64>> + '_ {
        Self::neighbors(coords).filter(|coords| visits.insert(*coords))
    }

    fn neighbors(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .values()
            .map(move |delta| coords + delta.coords.cast())
    }
}

#[derive(Default)]
struct ChunkLight([[[BlockLight; Chunk::DIM]; Chunk::DIM]; Chunk::DIM]);

impl Index<Point3<u8>> for ChunkLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<u8>) -> &Self::Output {
        &self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

impl IndexMut<Point3<u8>> for ChunkLight {
    fn index_mut(&mut self, coords: Point3<u8>) -> &mut Self::Output {
        &mut self.0[coords.x as usize][coords.y as usize][coords.z as usize]
    }
}

pub struct ChunkAreaLight([[[BlockLight; ChunkArea::DIM]; ChunkArea::DIM]; ChunkArea::DIM]);

impl ChunkAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 - ChunkArea::PADDING as i8)))
            })
        }))
    }

    pub fn block_area_light(&self, coords: Point3<u8>) -> BlockAreaLight {
        let coords = coords.cast();
        BlockAreaLight::from_fn(|delta| self[coords + delta.coords])
    }
}

impl Index<Point3<i8>> for ChunkAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| (c + ChunkArea::PADDING as i8) as usize);
        &self.0[coords.x][coords.y][coords.z]
    }
}

bitfield! {
    #[derive(Clone, Copy, Default)]
    pub struct BlockLight(u32);
    u8, component, set_component: 3, 0, 6;
}

impl BlockLight {
    const SKYLIGHT_RANGE: Range<usize> = 0..3;
    const TORCHLIGHT_RANGE: Range<usize> = 3..6;

    fn replace_component(&mut self, index: usize, value: u8) -> u8 {
        let component = self.component(index);
        self.set_component(index, value);
        component
    }
}

pub struct BlockAreaLight([[[BlockLight; BlockArea::DIM]; BlockArea::DIM]; BlockArea::DIM]);

impl BlockAreaLight {
    fn from_fn<F: FnMut(Point3<i8>) -> BlockLight>(mut f: F) -> Self {
        Self(array::from_fn(|x| {
            array::from_fn(|y| {
                array::from_fn(|z| f(point![x, y, z].map(|c| c as i8 - BlockArea::PADDING as i8)))
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

impl Index<Point3<i8>> for BlockAreaLight {
    type Output = BlockLight;

    fn index(&self, coords: Point3<i8>) -> &Self::Output {
        let coords = coords.map(|c| (c + BlockArea::PADDING as i8) as usize);
        &self.0[coords.x][coords.y][coords.z]
    }
}

struct BlockLightSum {
    sums: [u8; 6],
    count: u8,
}

impl BlockLightSum {
    fn avg(self) -> BlockLight {
        let mut value = BlockLight::default();
        for (i, sum) in self.sums.into_iter().enumerate() {
            value.set_component(i, sum / self.count.max(1))
        }
        value
    }
}

impl Sum<BlockLight> for BlockLightSum {
    fn sum<I: Iterator<Item = BlockLight>>(iter: I) -> Self {
        let (sums, count) = iter.fold(([0; 6], 0), |(sums, count), light| {
            (array::from_fn(|i| sums[i] + light.component(i)), count + 1)
        });
        Self { sums, count }
    }
}

struct LightNode {
    chunk_coords: Point3<i32>,
    block_coords: Point3<u8>,
}

impl LightNode {
    fn new(coords: Point3<i64>) -> Self {
        Self {
            chunk_coords: ChunkMap::chunk_coords(coords),
            block_coords: ChunkMap::block_coords(coords),
        }
    }

    fn apply_filter(
        &self,
        cells: &FxHashMap<Point3<i32>, ChunkCell>,
        index: usize,
        value: u8,
    ) -> u8 {
        (value as f32 * self.filter(cells, index)).round() as u8
    }

    fn filter(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>, index: usize) -> f32 {
        self.block_data(cells).light_filter[index % 3]
    }

    fn block_data(&self, cells: &FxHashMap<Point3<i32>, ChunkCell>) -> &'static BlockData {
        cells
            .get(&self.chunk_coords)
            .map_or(Block::Air, |cell| cell[self.block_coords])
            .data()
    }
}

use super::{
    action::BlockAction,
    block::{
        area::BlockAreaLight,
        data::{BlockData, SIDE_DELTAS},
        Block, BlockLight,
    },
    chunk::{
        area::{ChunkArea, ChunkAreaLight},
        Chunk, ChunkLight,
    },
    ChunkStore, World,
};
use crate::shared::{color::Rgb, utils};
use nalgebra::{point, vector, Point3, Vector3};
use rustc_hash::FxHashMap;
use std::{
    cmp::Ordering,
    collections::{hash_map::Entry, VecDeque},
    iter,
    ops::{Index, IndexMut, Range},
};

#[derive(Default)]
pub struct WorldLight(FxHashMap<Point3<i32>, ChunkLight>);

impl WorldLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let mut value = ChunkAreaLight::default();
        for delta in ChunkArea::chunk_deltas() {
            if let Some(light) = self.get(coords + delta) {
                for (coords, delta) in ChunkArea::block_deltas(delta) {
                    value[delta] = light[coords];
                }
            }
        }
        value
    }

    pub fn block_area_light(&self, coords: Point3<i64>) -> BlockAreaLight {
        BlockAreaLight::from_fn(|delta| self.block_light(coords + delta.cast()))
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> Vec<Point3<i64>> {
        match action {
            BlockAction::Place(block) => {
                let (skylight, height) = self.cast_light_beam(chunks, coords);
                let data = block.data();
                let value = BlockLight::new(skylight, data.luminance);
                let luminance = self.luminance(coords, value, Some(data));
                let mut work_area = WorkArea::new(coords, luminance, height);
                work_area.populate(chunks, self);
                work_area.place(coords, data);
                work_area.apply(self)
            }
            BlockAction::Destroy => {
                let (skylight, height) = self.cast_light_beam(chunks, coords);
                let value = self.flood(coords, skylight);
                let luminance = self.luminance(coords, value, None);
                let mut work_area = WorkArea::new(coords, luminance, height);
                work_area.populate(chunks, self);
                work_area.destroy(coords, value);
                work_area.apply(self)
            }
        }
    }

    fn luminance(&self, coords: Point3<i64>, value: BlockLight, data: Option<&BlockData>) -> u8 {
        if let Some(data) = data {
            iter::zip(self.block_light(coords), value)
                .enumerate()
                .map(|(i, (a, b))| {
                    if BlockLight::TORCHLIGHT_RANGE.contains(&i) {
                        if data.light_filter[i % 3] == 0 {
                            a.max(b)
                        } else if a < b {
                            b
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                })
                .max()
                .unwrap_or_else(|| unreachable!())
        } else {
            self.block_light(coords)
                .zip_map(value, |a, b| match a.cmp(&b) {
                    Ordering::Less => b,
                    Ordering::Equal => 0,
                    Ordering::Greater => a,
                })
                .max()
        }
    }

    fn cast_light_beam(&self, chunks: &ChunkStore, coords: Point3<i64>) -> (Rgb<u8>, u64) {
        let mut accum = Rgb::splat(BlockLight::COMPONENT_MAX);
        let mut value = accum;
        let mut bottom = utils::coords((World::Y_RANGE.start, 0));

        for (y, block) in chunks.column(coords.xz()) {
            match coords.y.cmp(&y) {
                Ordering::Less => {
                    accum *= block.data().light_filter;
                    value = accum;
                    if accum == Default::default() {
                        bottom = coords.y;
                        break;
                    }
                }
                Ordering::Equal => {}
                Ordering::Greater => {
                    accum *= block.data().light_filter;
                    if accum == Default::default() {
                        bottom = y + 1;
                        break;
                    }
                }
            }
        }

        (value, (coords.y - bottom) as u64)
    }

    fn flood(&self, coords: Point3<i64>, skylight: Rgb<u8>) -> BlockLight {
        WorkArea::adjacent_points(coords)
            .map(|coords| self.block_light(coords))
            .reduce(BlockLight::sup)
            .unwrap_or_else(|| unreachable!())
            .map(|c| c.saturating_sub(1))
            .sup(BlockLight::new(skylight, Default::default()))
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.get(utils::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[utils::block_coords(coords)])
    }

    fn get(&self, coords: Point3<i32>) -> Option<&ChunkLight> {
        self.0.get(&coords)
    }

    fn entry(&mut self, coords: Point3<i32>) -> Entry<Point3<i32>, ChunkLight> {
        self.0.entry(coords)
    }
}

struct WorkArea {
    data: Vec<(Block, BlockLight)>,
    min: Point3<i64>,
    dims: Vector3<i64>,
}

impl WorkArea {
    fn new(coords: Point3<i64>, luminance: u8, height: u64) -> Self {
        let radius = luminance as i64 - 1;
        let min = coords - Vector3::repeat(radius);
        let dims = Vector3::repeat(1 + radius * 2) + vector![0, height as i64, 0];
        let data = vec![Default::default(); dims.product().max(0) as usize];
        Self { data, min, dims }
    }

    fn populate(&mut self, chunks: &ChunkStore, light: &WorldLight) {
        for chunk_coords in self.chunk_points() {
            match (chunks.get(chunk_coords), light.get(chunk_coords)) {
                (Some(chunk), Some(light)) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))] =
                            (chunk[block_coords], light[block_coords]);
                    }
                }
                (Some(chunk), None) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))].0 = chunk[block_coords];
                    }
                }
                (None, Some(light)) => {
                    for block_coords in self.block_points(chunk_coords) {
                        self[utils::coords((chunk_coords, block_coords))].1 = light[block_coords];
                    }
                }
                (None, None) => {}
            }
        }
    }

    fn place(&mut self, coords: Point3<i64>, data: &BlockData) {
        if self.is_in_bounds(coords) {
            for i in BlockLight::TORCHLIGHT_RANGE {
                let value = data.luminance[i % 3];
                self.place_filter(coords, i, value, data.light_filter[i % 3]);
                self.place_component(coords, i, value);
            }
        }
    }

    fn destroy(&mut self, coords: Point3<i64>, value: BlockLight) {
        if self.is_in_bounds(coords) {
            for i in BlockLight::TORCHLIGHT_RANGE {
                self.destroy_component(coords, i, value.component(i));
            }
        }
    }

    fn apply(&self, light: &mut WorldLight) -> Vec<Point3<i64>> {
        let mut changes = vec![];
        for chunk_coords in self.chunk_points() {
            match light.entry(chunk_coords) {
                Entry::Occupied(mut entry) => {
                    for block_coords in self.block_points(chunk_coords) {
                        let coords = utils::coords((chunk_coords, block_coords));
                        if entry.get_mut().set(block_coords, self[coords].1) {
                            changes.push(coords);
                        }
                    }
                    if entry.get().is_empty() {
                        entry.remove();
                    }
                }
                Entry::Vacant(entry) => {
                    let mut non_zero_lights = vec![];
                    for block_coords in self.block_points(chunk_coords) {
                        let coords = utils::coords((chunk_coords, block_coords));
                        let light = self[coords].1;
                        if light != Default::default() {
                            non_zero_lights.push((block_coords, light));
                            changes.push(coords);
                        }
                    }
                    if !non_zero_lights.is_empty() {
                        let light = entry.insert(Default::default());
                        for (coords, non_zero_light) in non_zero_lights {
                            light.set(coords, non_zero_light);
                        }
                    }
                }
            }
        }
        changes
    }

    fn place_filter(&mut self, coords: Point3<i64>, index: usize, value: u8, filter: u8) {
        if filter == 0 {
            let (_, light) = &mut self[coords];
            let component = light.component(index);
            if component > value {
                light.set_component(index, 0);
                self.unspread_component(coords, index, component);
            }
        }
    }

    fn place_component(&mut self, coords: Point3<i64>, index: usize, value: u8) {
        let (_, light) = &mut self[coords];
        if light.component(index) < value {
            light.set_component(index, value);
            self.spread_component(coords, index, value);
        }
    }

    fn destroy_component(&mut self, coords: Point3<i64>, index: usize, value: u8) {
        let (_, light) = &mut self[coords];
        let component = light.component(index);
        match component.cmp(&value) {
            Ordering::Less => {
                light.set_component(index, value);
                self.spread_component(coords, index, value);
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                light.set_component(index, 0);
                self.unspread_component(coords, index, component);
            }
        }
    }

    fn unspread_component(&mut self, coords: Point3<i64>, index: usize, expected: u8) {
        let mut deq = VecDeque::from_iter(Self::neighbors(coords, expected));
        let mut sources = vec![];

        while let Some((coords, expected)) = deq.pop_front() {
            let (block, ref mut light) = self[coords];
            let component = light.component(index);
            let expected = expected * block.data().light_filter[index % 3];
            match component.cmp(&expected) {
                Ordering::Less => {}
                Ordering::Equal => {
                    let luminance = block.data().luminance[index % 3];
                    if luminance != 0 {
                        light.set_component(index, luminance);
                        sources.push((coords, luminance));
                    } else {
                        light.set_component(index, 0);
                    }
                    deq.extend(Self::neighbors(coords, expected));
                }
                Ordering::Greater => sources.push((coords, component)),
            }
        }

        for (coords, value) in sources {
            self.spread_component(coords, index, value);
        }
    }

    fn spread_component(&mut self, coords: Point3<i64>, index: usize, value: u8) {
        let mut deq = VecDeque::from_iter(Self::neighbors(coords, value));
        while let Some((coords, value)) = deq.pop_front() {
            if let Some((block, light)) = self.get_mut(coords) {
                let value = value * block.data().light_filter[index % 3];
                if light.component(index) < value {
                    light.set_component(index, value);
                    deq.extend(Self::neighbors(coords, value));
                }
            }
        }
    }

    fn chunk_points(&self) -> impl Iterator<Item = Point3<i32>> {
        let min = utils::chunk_coords(self.min);
        let max = utils::chunk_coords(self.max());
        (min.x..=max.x).flat_map(move |x| {
            (min.y..=max.y).flat_map(move |y| (min.z..=max.z).map(move |z| point![x, y, z]))
        })
    }

    fn block_points(&self, coords: Point3<i32>) -> impl Iterator<Item = Point3<u8>> {
        let min = self.min;
        let max = self.max();
        Self::block_axis_range(coords.x, min.x, max.x).flat_map(move |x| {
            Self::block_axis_range(coords.y, min.y, max.y).flat_map(move |y| {
                Self::block_axis_range(coords.z, min.z, max.z).map(move |z| point![x, y, z])
            })
        })
    }

    fn max(&self) -> Point3<i64> {
        self.min + self.dims - Vector3::repeat(1)
    }

    fn get(&self, coords: Point3<i64>) -> Option<&(Block, BlockLight)> {
        self.is_in_bounds(coords)
            .then(|| unsafe { self.data.get_unchecked(self.index_unchecked(coords)) })
    }

    fn get_mut(&mut self, coords: Point3<i64>) -> Option<&mut (Block, BlockLight)> {
        self.is_in_bounds(coords).then(|| unsafe {
            let idx = self.index_unchecked(coords);
            self.data.get_unchecked_mut(idx)
        })
    }

    fn is_in_bounds(&self, coords: Point3<i64>) -> bool {
        let max = self.max();
        (self.min.x..=max.x).contains(&coords.x)
            && (self.min.y..=max.y).contains(&coords.y)
            && (self.min.z..=max.z).contains(&coords.z)
    }

    unsafe fn index_unchecked(&self, coords: Point3<i64>) -> usize {
        let delta = coords - self.min;
        (delta.x * self.dims.y * self.dims.z + delta.y * self.dims.z + delta.z) as usize
    }

    fn block_axis_range(c: i32, min: i64, max: i64) -> Range<u8> {
        if utils::chunk_coords(min) == utils::chunk_coords(max) {
            utils::block_coords(min)..utils::block_coords(max) + 1
        } else if c == utils::chunk_coords(min) {
            utils::block_coords(min)..Chunk::DIM as u8
        } else if c == utils::chunk_coords(max) {
            0..utils::block_coords(max) + 1
        } else {
            0..Chunk::DIM as u8
        }
    }

    fn neighbors(coords: Point3<i64>, value: u8) -> impl Iterator<Item = (Point3<i64>, u8)> {
        (value > 1)
            .then(|| Self::adjacent_points(coords).map(move |coords| (coords, value - 1)))
            .into_iter()
            .flatten()
    }

    fn adjacent_points(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
    }
}

impl Index<Point3<i64>> for WorkArea {
    type Output = (Block, BlockLight);

    fn index(&self, coords: Point3<i64>) -> &Self::Output {
        self.get(coords).expect("coords out of bounds")
    }
}

impl IndexMut<Point3<i64>> for WorkArea {
    fn index_mut(&mut self, coords: Point3<i64>) -> &mut Self::Output {
        self.get_mut(coords).expect("coords out of bounds")
    }
}

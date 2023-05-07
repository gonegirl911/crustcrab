use super::{
    block::{data::BlockData, light::BlockLight, Block, SIDE_DELTAS},
    chunk::{
        light::{ChunkAreaLight, ChunkLight},
        Chunk, ChunkArea,
    },
    {BlockAction, ChunkStore, World},
};
use nalgebra::{point, Point3, Vector3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        hash_map::{Entry, VacantEntry},
        VecDeque,
    },
    num::NonZeroU8,
};

#[derive(Default)]
pub struct ChunkMapLight(FxHashMap<Point3<i32>, ChunkLight>);

impl ChunkMapLight {
    pub fn chunk_area_light(&self, coords: Point3<i32>) -> ChunkAreaLight {
        let mut value = ChunkAreaLight::default();
        for (chunk_delta, deltas) in ChunkArea::deltas() {
            let light = self.0.get(&(coords + chunk_delta));
            for (block_coords, delta) in deltas {
                value[delta] = light.map_or_else(Default::default, |light| light[block_coords]);
            }
        }
        value
    }

    pub fn insert_many<I>(&mut self, chunks: &ChunkStore, points: I) -> FxHashSet<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        Default::default()
    }

    pub fn remove_many<I>(&mut self, chunks: &ChunkStore, points: I) -> FxHashSet<Point3<i64>>
    where
        I: IntoIterator<Item = Point3<i32>>,
    {
        Default::default()
    }

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i64>> {
        match action {
            BlockAction::Place(block) => self.place(chunks, coords, block.data()),
            BlockAction::Destroy => self.destroy(chunks, coords),
        }
    }

    fn place(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        data: &BlockData,
    ) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        updates.extend(self.block_skylight(chunks, coords, data));
        updates.extend(self.place_torchlight(chunks, coords, data));
        updates
    }

    fn destroy(&mut self, chunks: &ChunkStore, coords: Point3<i64>) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        updates.extend(self.unblock_skylight(chunks, coords));
        updates.extend(self.destroy_torchlight(chunks, coords));
        updates
    }

    fn block_skylight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        data: &BlockData,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE
            .zip(data.light_filter)
            .flat_map(move |(i, f)| {
                let block_light = self.block_light_mut(coords);
                let component = block_light.component(i);
                let value = Self::apply_filter(component, f);
                if block_light.set_component(i, value) {
                    let mut updates = FxHashSet::from_iter([coords]);
                    updates.extend(self.unspread_component(chunks, coords, i, component, f));
                    let light_beam = Self::light_beam_value(chunks, coords + Vector3::y(), i);
                    updates.extend(self.unspread_light_beam(chunks, coords, i, light_beam, f));
                    updates
                } else {
                    Default::default()
                }
            })
    }

    fn place_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        data: &BlockData,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE
            .zip(data.luminance)
            .zip(data.light_filter)
            .flat_map(move |((i, l), f)| {
                let block_light = self.block_light_mut(coords);
                let component = block_light.component(i);
                let value = Self::apply_filter(component, f);
                if value >= l {
                    if block_light.set_component(i, value) {
                        Some(coords)
                            .into_iter()
                            .chain(vec![])
                            .chain(self.unspread_component(chunks, coords, i, component, f))
                    } else {
                        None.into_iter().chain(vec![]).chain(FxHashSet::default())
                    }
                } else {
                    block_light
                        .set_component(i, l)
                        .then_some(coords)
                        .into_iter()
                        .chain(self.spread_component(chunks, coords, i, l))
                        .chain(FxHashSet::default())
                }
            })
    }

    fn unblock_skylight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
            let neighbor = self.brightest_neighbor(coords, i);
            let block_light = self.block_light_mut(coords);
            let light_beam = Self::light_beam_value(chunks, coords + Vector3::y(), i);
            let value = neighbor.saturating_sub(1).max(light_beam);
            match block_light.component(i).cmp(&value) {
                Ordering::Less => {
                    block_light.set_component(i, value);
                    let mut updates = FxHashSet::from_iter([coords]);
                    updates.extend(self.spread_component(chunks, coords, i, value));
                    updates.extend(self.spread_light_beam(chunks, coords, i, light_beam));
                    updates
                }
                Ordering::Equal => Default::default(),
                Ordering::Greater => unreachable!(),
            }
        })
    }

    fn destroy_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE.flat_map(move |i| {
            let value = self.brightest_neighbor(coords, i).saturating_sub(1);
            let block_light = self.block_light_mut(coords);
            let component = block_light.component(i);
            match component.cmp(&value) {
                Ordering::Less => {
                    block_light.set_component(i, value);
                    Some(coords)
                        .into_iter()
                        .chain(self.spread_component(chunks, coords, i, value))
                        .chain(FxHashSet::default())
                }
                Ordering::Equal => None.into_iter().chain(vec![]).chain(FxHashSet::default()),
                Ordering::Greater => {
                    block_light.set_component(i, 0);
                    Some(coords)
                        .into_iter()
                        .chain(vec![])
                        .chain(self.unspread_component(chunks, coords, i, component, 0.0))
                }
            }
        })
    }

    fn spread_light_beam<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        index: usize,
        mut value: u8,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        Self::floor(coords)
            .map_while(
                move |coords| match self.set_component(chunks, coords, index, value) {
                    Ok(v) => {
                        value = v;
                        Some(
                            Some(coords)
                                .into_iter()
                                .chain(self.spread_component(chunks, coords, index, v)),
                        )
                    }
                    Err(0) => None,
                    Err(v) => {
                        value = v;
                        Some(None.into_iter().chain(vec![]))
                    }
                },
            )
            .flatten()
    }

    fn unspread_light_beam<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        index: usize,
        mut expected: u8,
        filter: f32,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        Self::floor(coords)
            .map_while(move |coords| {
                match self.unset_component(chunks, coords, index, expected, filter) {
                    Ok(None) | Err((_, 0)) => None,
                    Ok(Some(0)) => unreachable!(),
                    Ok(Some(e)) => {
                        expected = e;
                        Some(
                            Some(coords)
                                .into_iter()
                                .chain(self.unspread_component(chunks, coords, index, e, filter)),
                        )
                    }
                    Err((_, e)) => {
                        expected = e;
                        Some(None.into_iter().chain(FxHashSet::default()))
                    }
                }
            })
            .flatten()
    }

    fn spread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Vec<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut visits = FxHashSet::from_iter([coords]);
        let mut updates = vec![];

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                if let Ok(value) = self.set_component(chunks, coords, index, value - 1) {
                    deq.push_back((coords, value));
                    updates.push(coords);
                }
            }
        }

        updates
    }

    fn unspread_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        expected: u8,
        filter: f32,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, expected)]);
        let mut visits = FxHashSet::from_iter([coords]);
        let mut updates = vec![];
        let mut sources = vec![];

        while let Some((coords, expected)) = deq.pop_front() {
            for coords in Self::unvisited_neighbors(coords, &mut visits) {
                match self.unset_component(chunks, coords, index, expected - 1, filter) {
                    Ok(None) | Err((Ok(0) | Err(Ok(0) | Err(0)), _)) => {}
                    Ok(Some(0)) => unreachable!(),
                    Ok(Some(expected)) => {
                        deq.push_back((coords, expected));
                        updates.push(coords);
                    }
                    Err((Ok(component), _)) => {
                        sources.push((coords, component));
                    }
                    Err((Err(Ok(luminance)), _)) => {
                        updates.push(coords);
                        sources.push((coords, luminance));
                    }
                    Err((Err(Err(luminance)), _)) => {
                        sources.push((coords, luminance));
                    }
                }
            }
        }

        sources
            .into_iter()
            .flat_map(|(coords, value)| self.spread_component(chunks, coords, index, value))
            .chain(updates)
            .collect()
    }

    fn component(&self, coords: Point3<i64>, index: usize) -> u8 {
        self.block_light(coords).component(index)
    }

    fn set_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
    ) -> Result<u8, u8> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        let data = chunks.block(coords).data();
        let value = Self::apply_filter(value, Self::filter(data, index));
        if component < value {
            block_light.set_component(index, value);
            Ok(value)
        } else {
            Err(value)
        }
    }

    #[allow(clippy::type_complexity)]
    fn unset_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        expected: u8,
        filter: f32,
    ) -> Result<Option<u8>, (Result<u8, Result<u8, u8>>, u8)> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.component(index);
        let data = chunks.block(coords).data();
        let expected = Self::apply_filter(expected, Self::filter(data, index));
        match component.cmp(&expected) {
            Ordering::Less => unreachable!(),
            Ordering::Equal => {
                let value = Self::apply_filter(component, filter);
                let luminance = Self::luminance(data, index);
                if value >= luminance {
                    Ok(block_light.set_component(index, value).then_some(expected))
                } else if block_light.set_component(index, luminance) {
                    Err((Err(Ok(luminance)), expected))
                } else {
                    Err((Err(Err(luminance)), expected))
                }
            }
            Ordering::Greater => Err((Ok(component), expected)),
        }
    }

    fn brightest_neighbor(&self, coords: Point3<i64>, index: usize) -> u8 {
        Self::neighbors(coords)
            .map(|coords| self.component(coords, index))
            .max()
            .unwrap_or_else(|| unreachable!())
    }

    fn block_light(&self, coords: Point3<i64>) -> BlockLight {
        self.0
            .get(&World::chunk_coords(coords))
            .map_or_else(Default::default, |light| light[World::block_coords(coords)])
    }

    fn block_light_mut(&mut self, coords: Point3<i64>) -> BlockLightRefMut {
        BlockLightRefMut::new(
            self.0.entry(World::chunk_coords(coords)),
            World::block_coords(coords),
        )
    }

    fn light_beam_value(chunks: &ChunkStore, coords: Point3<i64>, index: usize) -> u8 {
        let [chunk_x, chunk_z] = <[_; 2]>::from(World::chunk_coords(coords.xz()));
        let [block_x, block_z] = <[_; 2]>::from(World::block_coords(coords.xz()));
        World::Y_RANGE
            .rev()
            .filter_map(|y| Some((y, chunks.get(point![chunk_x, y, chunk_z])?)))
            .flat_map(|(chunk_y, chunk)| {
                (0..Chunk::DIM as u8)
                    .rev()
                    .map(move |y| (chunk_y, y, chunk[point![block_x, y, block_z]]))
            })
            .filter(|(_, _, block)| *block != Block::Air)
            .take_while(|(chunk_y, block_y, _)| {
                World::coords(point![*chunk_y], point![*block_y]).x > coords.y
            })
            .map(|(_, _, block)| Self::filter(block.data(), index))
            .try_fold(BlockLight::COMPONENT_MAX, |accum, f| {
                Some(NonZeroU8::new(Self::apply_filter(accum, f))?.get())
            })
            .unwrap_or(0)
    }

    fn floor(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        let bottom = World::coords(point![World::Y_RANGE.start], Default::default()).x;
        (bottom..coords.y)
            .rev()
            .map(move |y| point![coords.x, y, coords.z])
    }

    fn unvisited_neighbors(
        coords: Point3<i64>,
        visits: &mut FxHashSet<Point3<i64>>,
    ) -> impl Iterator<Item = Point3<i64>> + '_ {
        Self::neighbors(coords).filter(|coords| visits.insert(*coords))
    }

    fn neighbors(coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> {
        SIDE_DELTAS
            .into_values()
            .map(move |delta| coords + delta.cast())
    }

    fn apply_filter(value: u8, filter: f32) -> u8 {
        (value as f32 * filter).round() as u8
    }

    fn luminance(data: &BlockData, index: usize) -> u8 {
        if BlockLight::TORCHLIGHT_RANGE.contains(&index) {
            data.luminance[index - BlockLight::TORCHLIGHT_RANGE.start]
        } else {
            0
        }
    }

    fn filter(data: &BlockData, index: usize) -> f32 {
        data.light_filter[index % 3]
    }
}

enum BlockLightRefMut<'a> {
    Occupied(&'a mut BlockLight),
    Vacant {
        entry: VacantEntry<'a, Point3<i32>, ChunkLight>,
        coords: Point3<u8>,
    },
}

impl<'a> BlockLightRefMut<'a> {
    fn new(entry: Entry<'a, Point3<i32>, ChunkLight>, coords: Point3<u8>) -> Self {
        match entry {
            Entry::Occupied(entry) => Self::Occupied(&mut entry.into_mut()[coords]),
            Entry::Vacant(entry) => Self::Vacant { entry, coords },
        }
    }

    fn component(&self, index: usize) -> u8 {
        self.get().component(index)
    }

    fn set_component(self, index: usize, value: u8) -> bool {
        if self.component(index) != value {
            self.into_mut().set_component(index, value);
            true
        } else {
            false
        }
    }

    fn get(&self) -> BlockLight {
        if let Self::Occupied(block_light) = self {
            **block_light
        } else {
            Default::default()
        }
    }

    fn into_mut(self) -> &'a mut BlockLight {
        match self {
            Self::Occupied(block_light) => block_light,
            Self::Vacant { entry, coords } => &mut entry.insert(Default::default())[coords],
        }
    }
}

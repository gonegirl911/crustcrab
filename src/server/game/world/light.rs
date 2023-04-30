use super::{
    block::{data::BlockData, light::BlockLight, Block, SIDE_DELTAS},
    chunk::{
        light::{ChunkAreaLight, ChunkLight},
        ChunkArea,
    },
    {BlockAction, ChunkStore, World},
};
use nalgebra::{point, Point3};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Ordering,
    collections::{
        hash_map::{Entry, VacantEntry},
        VecDeque,
    },
    iter,
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

    pub fn apply(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        action: &BlockAction,
    ) -> FxHashSet<Point3<i64>> {
        match action {
            BlockAction::Place(block) => self.place(chunks, coords, *block),
            BlockAction::Destroy => self.destroy(chunks, coords),
        }
    }

    fn place(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> FxHashSet<Point3<i64>> {
        let mut updates = FxHashSet::default();
        updates.extend(self.block_skylight(chunks, coords));
        updates.extend(self.place_torchlight(chunks, coords, block));
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
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
            if let Some(value) = Self::light_beam_value(chunks, coords, i) {
                let mut updates = FxHashSet::default();
                updates.extend(self.unspread_light_beam(chunks, coords, i, value));
                updates.extend(self.spread_light_beam(chunks, coords, i, value));
                updates
            } else {
                let component = self.take_component(coords, i);
                if component != 0 {
                    self.unspread_component(chunks, coords, i, component)
                } else {
                    Default::default()
                }
            }
        })
    }

    fn place_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
        block: Block,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE
            .zip(block.data().luminance)
            .flat_map(move |(i, v)| {
                let component = self.replace_component(coords, i, v);
                match component.cmp(&v) {
                    Ordering::Less => self.spread_component(chunks, coords, i, v),
                    Ordering::Equal => Default::default(),
                    Ordering::Greater => self.unspread_component(chunks, coords, i, component),
                }
            })
    }

    fn unblock_skylight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::SKYLIGHT_RANGE.flat_map(move |i| {
            if let Some(value) = Self::light_beam_value(chunks, coords, i) {
                self.spread_light_beam(chunks, coords, i, value).collect()
            } else {
                self.fill_component(chunks, coords, i)
            }
        })
    }

    fn destroy_torchlight<'a>(
        &'a mut self,
        chunks: &'a ChunkStore,
        coords: Point3<i64>,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        BlockLight::TORCHLIGHT_RANGE.flat_map(move |i| {
            let component = self.take_component(coords, i);
            if component != 0 {
                self.unspread_component(chunks, coords, i, component)
            } else {
                self.fill_component(chunks, coords, i)
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
        Self::floor(chunks, coords)
            .map_while(
                move |coords| match self.set_component(chunks, coords, index, value) {
                    Ok(v) => {
                        value = v;
                        Some(self.spread_component(chunks, coords, index, v))
                    }
                    Err(0) => None,
                    Err(v) => {
                        value = v;
                        Some(Default::default())
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
        mut value: u8,
    ) -> impl Iterator<Item = Point3<i64>> + 'a {
        Self::floor(chunks, coords)
            .zip(iter::once(false).chain(iter::repeat(true)))
            .map_while(move |(coords, apply_filter)| {
                match self.unset_component(chunks, coords, index, value, apply_filter) {
                    Ok((v, _)) => {
                        value = v;
                        Some(self.unspread_component(chunks, coords, index, v))
                    }
                    Err((_, 0)) => None,
                    Err((_, v)) => {
                        value = v;
                        Some(Default::default())
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
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::from_iter([coords]);

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                if let Ok(value) = self.set_component(chunks, coords, index, value - 1) {
                    deq.push_back((coords, value));
                    updates.insert(coords);
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
        value: u8,
    ) -> FxHashSet<Point3<i64>> {
        let mut deq = VecDeque::from([(coords, value)]);
        let mut updates = FxHashSet::from_iter([coords]);
        let mut sources = FxHashSet::default();

        while let Some((coords, value)) = deq.pop_front() {
            for coords in Self::neighbors(coords) {
                match self.unset_component(chunks, coords, index, value - 1, true) {
                    Ok((value, 0)) => {
                        deq.push_back((coords, value));
                        updates.insert(coords);
                    }
                    Ok((value, luminance)) => {
                        deq.push_back((coords, value));
                        updates.insert(coords);
                        sources.insert((coords, luminance));
                    }
                    Err((0, _)) => {}
                    Err((component, _)) => {
                        sources.insert((coords, component));
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

    fn fill_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
    ) -> FxHashSet<Point3<i64>> {
        let value = self.brightest_neighbor(coords, index);
        if value > 1 {
            self.overwrite_component(coords, index, value - 1);
            self.spread_component(chunks, coords, index, value - 1)
        } else {
            Default::default()
        }
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
        let component = block_light.get().component(index);
        let data = chunks.block(coords).data();
        let value = Self::apply_filter(value, Self::filter(data, index));
        if component < value {
            block_light.into_mut().set_component(index, value);
            Ok(value)
        } else {
            Err(value)
        }
    }

    fn unset_component(
        &mut self,
        chunks: &ChunkStore,
        coords: Point3<i64>,
        index: usize,
        value: u8,
        apply_filter: bool,
    ) -> Result<(u8, u8), (u8, u8)> {
        let block_light = self.block_light_mut(coords);
        let component = block_light.get().component(index);
        let data = chunks.block(coords).data();
        let value = if apply_filter {
            Self::apply_filter(value, Self::filter(data, index))
        } else {
            value
        };
        if component != 0 && component == value {
            let luminance = Self::luminance(data, index);
            block_light.into_mut().set_component(index, luminance);
            Ok((value, luminance))
        } else {
            Err((component, value))
        }
    }

    fn overwrite_component(&mut self, coords: Point3<i64>, index: usize, value: u8) {
        self.block_light_mut(coords)
            .into_mut()
            .set_component(index, value)
    }

    fn replace_component(&mut self, coords: Point3<i64>, index: usize, value: u8) -> u8 {
        let block_light = self.block_light_mut(coords);
        let component = block_light.get().component(index);
        if component != value {
            block_light.into_mut().set_component(index, value);
        }
        component
    }

    fn take_component(&mut self, coords: Point3<i64>, index: usize) -> u8 {
        self.replace_component(coords, index, 0)
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

    fn block_light_mut(&mut self, coords: Point3<i64>) -> BlockLightRefMut<'_> {
        BlockLightRefMut::new(
            self.0.entry(World::chunk_coords(coords)),
            World::block_coords(coords),
        )
    }

    fn light_beam_value(chunks: &ChunkStore, coords: Point3<i64>, index: usize) -> Option<u8> {
        let top = chunks.top(coords.xz()).unwrap_or(i64::MIN);
        (coords.y + 1..=top)
            .rev()
            .map(|y| Self::filter(chunks.block(point![coords.x, y, coords.z]).data(), index))
            .try_fold(BlockLight::COMPONENT_MAX, |accum, f| {
                Some(NonZeroU8::new(Self::apply_filter(accum, f))?.get())
            })
    }

    fn floor(chunks: &ChunkStore, coords: Point3<i64>) -> impl Iterator<Item = Point3<i64>> + '_ {
        let bottom = chunks.bottom(coords.xz()).unwrap_or(i64::MAX);
        (bottom..=coords.y)
            .rev()
            .map(move |y| point![coords.x, y, coords.z])
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
